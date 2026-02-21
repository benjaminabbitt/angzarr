package angzarr

import (
	"context"
	"errors"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/types/known/anypb"
)

// CommandRejectedError indicates a command was rejected due to business rule violation.
// Maps to gRPC FAILED_PRECONDITION.
type CommandRejectedError struct {
	Message string
}

func (e CommandRejectedError) Error() string {
	return e.Message
}

// NewCommandRejectedError creates a new command rejected error.
func NewCommandRejectedError(msg string) error {
	return CommandRejectedError{Message: msg}
}

// StatePacker converts aggregate state to protobuf Any for Replay RPC.
// Used by MERGE_COMMUTATIVE strategy for conflict detection.
type StatePacker[S any] func(state S) (*anypb.Any, error)

// AggregateHandler wraps a CommandRouter for the gRPC Aggregate service.
//
// Maps domain errors to gRPC status codes:
//   - CommandRejectedError -> FAILED_PRECONDITION
//   - Other errors -> INVALID_ARGUMENT
type AggregateHandler[S any] struct {
	pb.UnimplementedAggregateServiceServer
	router      *CommandRouter[S]
	statePacker StatePacker[S]
}

// NewAggregateHandler creates a new aggregate handler with the given router.
func NewAggregateHandler[S any](router *CommandRouter[S]) *AggregateHandler[S] {
	return &AggregateHandler[S]{router: router}
}

// WithReplay enables Replay RPC support by providing a state packer.
//
// The state packer converts the aggregate's internal state to a protobuf Any
// message. This is required for MERGE_COMMUTATIVE strategy, which uses Replay
// to compute state diffs for conflict detection.
//
// Example:
//
//	func packPlayerState(state PlayerState) (*anypb.Any, error) {
//	    protoState := state.ToProto()
//	    return anypb.New(protoState)
//	}
//
//	handler := NewAggregateHandler(router).WithReplay(packPlayerState)
func (h *AggregateHandler[S]) WithReplay(packer StatePacker[S]) *AggregateHandler[S] {
	h.statePacker = packer
	return h
}

// Handle processes a contextual command asynchronously.
func (h *AggregateHandler[S]) Handle(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

// HandleSync processes a contextual command synchronously.
func (h *AggregateHandler[S]) HandleSync(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

func (h *AggregateHandler[S]) dispatch(req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	resp, err := h.router.Dispatch(req)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return resp, nil
}

// Replay computes state from events for MERGE_COMMUTATIVE conflict detection.
//
// Only available if WithReplay was called with a state packer.
// Returns UNIMPLEMENTED if no state packer is configured.
func (h *AggregateHandler[S]) Replay(ctx context.Context, req *pb.ReplayRequest) (*pb.ReplayResponse, error) {
	if h.statePacker == nil {
		return nil, status.Error(codes.Unimplemented,
			"Replay not implemented. Call WithReplay() to enable for MERGE_COMMUTATIVE strategy.")
	}

	// Build EventBook from ReplayRequest
	eventBook := &pb.EventBook{
		Pages:    req.Events,
		Snapshot: req.BaseSnapshot,
	}

	// Rebuild state using the router's state rebuilder
	state := h.router.RebuildState(eventBook)

	// Pack state to Any
	stateAny, err := h.statePacker(state)
	if err != nil {
		return nil, status.Error(codes.Internal, err.Error())
	}

	return &pb.ReplayResponse{State: stateAny}, nil
}

// RegisterAggregateHandler returns a ServiceRegistrar that registers an aggregate handler.
func RegisterAggregateHandler[S any](router *CommandRouter[S]) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterAggregateServiceServer(server, NewAggregateHandler(router))
	}
}

// RunAggregateServer starts a gRPC server for an aggregate.
//
// Parameters:
//   - domain: The aggregate's domain name
//   - defaultPort: Default TCP port if PORT env not set
//   - router: CommandRouter with registered handlers
func RunAggregateServer[S any](domain, defaultPort string, router *CommandRouter[S]) {
	RunServer(RegisterAggregateHandler(router), ServerOptions{
		ServiceName:      "Aggregate",
		Domain:           domain,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// SagaHandler wraps an EventRouter for the gRPC Saga service.
//
// Maps domain errors to gRPC status codes.
type SagaHandler struct {
	pb.UnimplementedSagaServiceServer
	router *EventRouter
}

// NewSagaHandler creates a new saga handler with the given router.
func NewSagaHandler(router *EventRouter) *SagaHandler {
	return &SagaHandler{router: router}
}

// Prepare declares which destination aggregates the saga needs to read.
// This is phase 1 of the two-phase saga protocol.
func (h *SagaHandler) Prepare(ctx context.Context, req *pb.SagaPrepareRequest) (*pb.SagaPrepareResponse, error) {
	destinations := h.router.PrepareDestinations(req.Source)
	return &pb.SagaPrepareResponse{Destinations: destinations}, nil
}

// Execute processes events and returns commands for other aggregates.
// This is phase 2 of the two-phase saga protocol.
func (h *SagaHandler) Execute(ctx context.Context, req *pb.SagaExecuteRequest) (*pb.SagaResponse, error) {
	commands, err := h.router.Dispatch(req.Source, req.Destinations)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return &pb.SagaResponse{Commands: commands}, nil
}

// RegisterSagaHandler returns a ServiceRegistrar that registers a saga handler.
func RegisterSagaHandler(router *EventRouter) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterSagaServiceServer(server, NewSagaHandler(router))
	}
}

// RunSagaServer starts a gRPC server for a saga.
//
// Parameters:
//   - name: The saga's name (e.g., "saga-order-fulfillment")
//   - defaultPort: Default TCP port if PORT env not set
//   - router: EventRouter with registered handlers
func RunSagaServer(name, defaultPort string, router *EventRouter) {
	RunServer(RegisterSagaHandler(router), ServerOptions{
		ServiceName:      "Saga",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// ProjectorHandleFunc processes an EventBook and returns a Projection.
type ProjectorHandleFunc func(events *pb.EventBook) (*pb.Projection, error)

// ProjectorHandler wraps a handle function for the gRPC Projector service.
type ProjectorHandler struct {
	pb.UnimplementedProjectorServiceServer
	name     string
	domains  []string
	handleFn ProjectorHandleFunc
}

// NewProjectorHandler creates a new projector handler.
func NewProjectorHandler(name string, domains ...string) *ProjectorHandler {
	return &ProjectorHandler{
		name:    name,
		domains: domains,
	}
}

// WithHandle sets the event handling callback.
func (h *ProjectorHandler) WithHandle(fn ProjectorHandleFunc) *ProjectorHandler {
	h.handleFn = fn
	return h
}

// Handle processes an EventBook and returns a Projection.
func (h *ProjectorHandler) Handle(ctx context.Context, req *pb.EventBook) (*pb.Projection, error) {
	if h.handleFn != nil {
		return h.handleFn(req)
	}
	return &pb.Projection{}, nil
}

// HandleSpeculative processes events without side effects.
func (h *ProjectorHandler) HandleSpeculative(ctx context.Context, req *pb.EventBook) (*pb.Projection, error) {
	return h.Handle(ctx, req)
}

// RegisterProjectorHandler returns a ServiceRegistrar that registers a projector handler.
func RegisterProjectorHandler(handler *ProjectorHandler) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterProjectorServiceServer(server, handler)
	}
}

// RunProjectorServer starts a gRPC server for a projector.
//
// Parameters:
//   - name: The projector's name (e.g., "prj-output")
//   - defaultPort: Default TCP port if PORT env not set
//   - handler: ProjectorHandler with configured handle function
func RunProjectorServer(name, defaultPort string, handler *ProjectorHandler) {
	RunServer(RegisterProjectorHandler(handler), ServerOptions{
		ServiceName:      "Projector",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// PMPrepareFunc declares additional destinations needed beyond the trigger.
type PMPrepareFunc func(trigger, processState *pb.EventBook) []*pb.Cover

// PMHandleFunc processes events and returns commands and process events.
type PMHandleFunc func(trigger, processState *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, *pb.EventBook, error)

// PMRevocationFunc handles saga/PM compensation for commands issued by this PM.
// Called when a command produced by this PM is rejected by the target aggregate.
//
// Parameters:
//   - notification: The Notification with RejectionNotification payload
//   - processState: Current PM state
//
// Returns: PMRevocationResponse with optional PM events and RevocationResponse
//
// To access rejection details:
//
//	ctx := NewCompensationContext(notification)
type PMRevocationFunc func(notification *pb.Notification, processState *pb.EventBook) *PMRevocationResponse

// ProcessManagerHandler wraps functions for the gRPC ProcessManager service.
type ProcessManagerHandler struct {
	pb.UnimplementedProcessManagerServiceServer
	name          string
	prepareFn     PMPrepareFunc
	handleFn      PMHandleFunc
	revocationFn  PMRevocationFunc
}

// NewProcessManagerHandler creates a new process manager handler.
func NewProcessManagerHandler(name string) *ProcessManagerHandler {
	return &ProcessManagerHandler{
		name: name,
	}
}

// WithPrepare sets the prepare callback.
func (h *ProcessManagerHandler) WithPrepare(fn PMPrepareFunc) *ProcessManagerHandler {
	h.prepareFn = fn
	return h
}

// WithHandle sets the handle callback.
func (h *ProcessManagerHandler) WithHandle(fn PMHandleFunc) *ProcessManagerHandler {
	h.handleFn = fn
	return h
}

// WithRevocationHandler sets the handler for saga compensation requests.
//
// Called when a command produced by this PM is rejected by the target aggregate.
// The handler should decide whether to:
// 1. Emit PM events to record the failure (return with ProcessEvents)
// 2. Delegate to framework (return with RevocationResponse only)
// 3. Both (return with ProcessEvents and RevocationResponse)
//
// If no handler is set, revocations delegate to framework by default.
func (h *ProcessManagerHandler) WithRevocationHandler(fn PMRevocationFunc) *ProcessManagerHandler {
	h.revocationFn = fn
	return h
}

// Prepare declares which additional destinations are needed.
func (h *ProcessManagerHandler) Prepare(ctx context.Context, req *pb.ProcessManagerPrepareRequest) (*pb.ProcessManagerPrepareResponse, error) {
	if h.prepareFn != nil {
		destinations := h.prepareFn(req.Trigger, req.ProcessState)
		return &pb.ProcessManagerPrepareResponse{Destinations: destinations}, nil
	}
	return &pb.ProcessManagerPrepareResponse{}, nil
}

// Handle processes events and returns commands and process events.
func (h *ProcessManagerHandler) Handle(ctx context.Context, req *pb.ProcessManagerHandleRequest) (*pb.ProcessManagerHandleResponse, error) {
	if h.handleFn != nil {
		commands, processEvents, err := h.handleFn(req.Trigger, req.ProcessState, req.Destinations)
		if err != nil {
			return nil, status.Error(codes.InvalidArgument, err.Error())
		}
		return &pb.ProcessManagerHandleResponse{
			Commands:      commands,
			ProcessEvents: processEvents,
		}, nil
	}
	return &pb.ProcessManagerHandleResponse{}, nil
}

// RegisterProcessManagerHandler returns a ServiceRegistrar that registers a process manager handler.
func RegisterProcessManagerHandler(handler *ProcessManagerHandler) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterProcessManagerServiceServer(server, handler)
	}
}

// RunProcessManagerServer starts a gRPC server for a process manager.
//
// Parameters:
//   - name: The PM's name (e.g., "pmg-hand-flow")
//   - defaultPort: Default TCP port if PORT env not set
//   - handler: ProcessManagerHandler with configured callbacks
func RunProcessManagerServer(name, defaultPort string, handler *ProcessManagerHandler) {
	RunServer(RegisterProcessManagerHandler(handler), ServerOptions{
		ServiceName:      "ProcessManager",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// ============================================================================
// OO-Style Handlers
// ============================================================================

// OOAggregate interface for OO-style aggregates.
// Implemented by types that embed AggregateBase.
type OOAggregate[S any] interface {
	Domain() string
	Handle(request *pb.ContextualCommand) (*pb.BusinessResponse, error)
	HandlerTypes() []string
}

// OOAggregateFactory creates a new OO aggregate instance with prior events.
type OOAggregateFactory[S any, A OOAggregate[S]] func(events *pb.EventBook) A

// OOAggregateHandler wraps an OO-style aggregate for the gRPC Aggregate service.
//
// Unlike the functional AggregateHandler, this creates a new aggregate instance
// for each request, passing in the prior events for state reconstruction.
//
// Example:
//
//	factory := func(events *pb.EventBook) *Table {
//	    return NewTable(events)
//	}
//	handler := NewOOAggregateHandler("table", factory)
type OOAggregateHandler[S any, A OOAggregate[S]] struct {
	pb.UnimplementedAggregateServiceServer
	domain  string
	factory OOAggregateFactory[S, A]
}

// NewOOAggregateHandler creates a new OO aggregate handler.
//
// Parameters:
//   - domain: The aggregate's domain name
//   - factory: Function to create a new aggregate with prior events
func NewOOAggregateHandler[S any, A OOAggregate[S]](domain string, factory OOAggregateFactory[S, A]) *OOAggregateHandler[S, A] {
	return &OOAggregateHandler[S, A]{
		domain:  domain,
		factory: factory,
	}
}

// Handle processes a contextual command asynchronously.
func (h *OOAggregateHandler[S, A]) Handle(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

// HandleSync processes a contextual command synchronously.
func (h *OOAggregateHandler[S, A]) HandleSync(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

func (h *OOAggregateHandler[S, A]) dispatch(req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	// Create aggregate with prior events
	agg := h.factory(req.Events)

	// Dispatch command
	resp, err := agg.Handle(req)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return resp, nil
}

// RegisterOOAggregateHandler returns a ServiceRegistrar that registers an OO aggregate handler.
func RegisterOOAggregateHandler[S any, A OOAggregate[S]](domain string, factory OOAggregateFactory[S, A]) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterAggregateServiceServer(server, NewOOAggregateHandler(domain, factory))
	}
}

// RunOOAggregateServer starts a gRPC server for an OO-style aggregate.
//
// Parameters:
//   - domain: The aggregate's domain name
//   - defaultPort: Default TCP port if PORT env not set
//   - factory: Function to create a new aggregate with prior events
func RunOOAggregateServer[S any, A OOAggregate[S]](domain, defaultPort string, factory OOAggregateFactory[S, A]) {
	RunServer(RegisterOOAggregateHandler(domain, factory), ServerOptions{
		ServiceName:      "Aggregate",
		Domain:           domain,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// OOSaga interface for OO-style sagas.
// Implemented by types that embed SagaBase.
type OOSaga interface {
	Name() string
	InputDomain() string
	OutputDomain() string
	PrepareDestinations(source *pb.EventBook) []*pb.Cover
	Execute(source *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, error)
}

// OOSagaHandler wraps an OO-style saga for the gRPC Saga service.
type OOSagaHandler struct {
	pb.UnimplementedSagaServiceServer
	saga OOSaga
}

// NewOOSagaHandler creates a new OO saga handler.
func NewOOSagaHandler(saga OOSaga) *OOSagaHandler {
	return &OOSagaHandler{saga: saga}
}

// Prepare declares which destination aggregates the saga needs to read.
func (h *OOSagaHandler) Prepare(ctx context.Context, req *pb.SagaPrepareRequest) (*pb.SagaPrepareResponse, error) {
	destinations := h.saga.PrepareDestinations(req.Source)
	return &pb.SagaPrepareResponse{Destinations: destinations}, nil
}

// Execute processes events and returns commands for other aggregates.
func (h *OOSagaHandler) Execute(ctx context.Context, req *pb.SagaExecuteRequest) (*pb.SagaResponse, error) {
	commands, err := h.saga.Execute(req.Source, req.Destinations)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return &pb.SagaResponse{Commands: commands}, nil
}

// RegisterOOSagaHandler returns a ServiceRegistrar that registers an OO saga handler.
func RegisterOOSagaHandler(saga OOSaga) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterSagaServiceServer(server, NewOOSagaHandler(saga))
	}
}

// RunOOSagaServer starts a gRPC server for an OO-style saga.
//
// Parameters:
//   - name: The saga's name (e.g., "saga-table-hand")
//   - defaultPort: Default TCP port if PORT env not set
//   - saga: OO saga instance
func RunOOSagaServer(name, defaultPort string, saga OOSaga) {
	RunServer(RegisterOOSagaHandler(saga), ServerOptions{
		ServiceName:      "Saga",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// OOProcessManager interface for OO-style process managers.
// Implemented by types that embed ProcessManagerBase.
type OOProcessManager interface {
	Name() string
	PMDomain() string
	InputDomains() []string
	PrepareDestinations(trigger, processState *pb.EventBook) []*pb.Cover
	Handle(trigger, processState *pb.EventBook, destinations []*pb.EventBook) ([]*pb.CommandBook, *pb.EventBook, *pb.Notification, error)
}

// OOProcessManagerHandler wraps an OO-style process manager for the gRPC ProcessManager service.
type OOProcessManagerHandler struct {
	pb.UnimplementedProcessManagerServiceServer
	pm OOProcessManager
}

// NewOOProcessManagerHandler creates a new OO process manager handler.
func NewOOProcessManagerHandler(pm OOProcessManager) *OOProcessManagerHandler {
	return &OOProcessManagerHandler{pm: pm}
}

// Prepare declares which additional destinations are needed.
func (h *OOProcessManagerHandler) Prepare(ctx context.Context, req *pb.ProcessManagerPrepareRequest) (*pb.ProcessManagerPrepareResponse, error) {
	destinations := h.pm.PrepareDestinations(req.Trigger, req.ProcessState)
	return &pb.ProcessManagerPrepareResponse{Destinations: destinations}, nil
}

// Handle processes events and returns commands and process events.
func (h *OOProcessManagerHandler) Handle(ctx context.Context, req *pb.ProcessManagerHandleRequest) (*pb.ProcessManagerHandleResponse, error) {
	commands, processEvents, _, err := h.pm.Handle(req.Trigger, req.ProcessState, req.Destinations)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return &pb.ProcessManagerHandleResponse{
		Commands:      commands,
		ProcessEvents: processEvents,
	}, nil
}

// RegisterOOProcessManagerHandler returns a ServiceRegistrar that registers an OO process manager handler.
func RegisterOOProcessManagerHandler(pm OOProcessManager) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterProcessManagerServiceServer(server, NewOOProcessManagerHandler(pm))
	}
}

// RunOOProcessManagerServer starts a gRPC server for an OO-style process manager.
//
// Parameters:
//   - name: The PM's name (e.g., "hand-flow")
//   - defaultPort: Default TCP port if PORT env not set
//   - pm: OO process manager instance
func RunOOProcessManagerServer(name, defaultPort string, pm OOProcessManager) {
	RunServer(RegisterOOProcessManagerHandler(pm), ServerOptions{
		ServiceName:      "ProcessManager",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}
