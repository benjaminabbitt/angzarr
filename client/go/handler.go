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
//
// # Why FAILED_PRECONDITION?
//
// Maps to gRPC FAILED_PRECONDITION because:
// 1. It signals the client SHOULD retry after updating state (fetching fresh events)
// 2. Distinguishes from INVALID_ARGUMENT (bad input, don't retry)
// 3. Matches the framework's retry policy which retries FAILED_PRECONDITION
//
// Use this for business rule rejections where the aggregate's current state
// doesn't allow the operation (e.g., "insufficient funds", "player already exists").
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

// CommandHandlerGrpc wraps a CommandRouter for the gRPC CommandHandler service.
//
// Maps domain errors to gRPC status codes:
//   - CommandRejectedError -> FAILED_PRECONDITION
//   - Other errors -> INVALID_ARGUMENT
type CommandHandlerGrpc[S any] struct {
	pb.UnimplementedCommandHandlerServiceServer
	router      *CommandRouter[S]
	statePacker StatePacker[S]
}

// NewCommandHandlerGrpc creates a new command handler with the given router.
func NewCommandHandlerGrpc[S any](router *CommandRouter[S]) *CommandHandlerGrpc[S] {
	return &CommandHandlerGrpc[S]{router: router}
}

// WithReplay enables Replay RPC support by providing a state packer.
//
// The state packer converts the command handler's internal state to a protobuf Any
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
//	handler := NewCommandHandlerGrpc(router).WithReplay(packPlayerState)
func (h *CommandHandlerGrpc[S]) WithReplay(packer StatePacker[S]) *CommandHandlerGrpc[S] {
	h.statePacker = packer
	return h
}

// Handle processes a contextual command asynchronously.
func (h *CommandHandlerGrpc[S]) Handle(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

// HandleSync processes a contextual command synchronously.
func (h *CommandHandlerGrpc[S]) HandleSync(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

func (h *CommandHandlerGrpc[S]) dispatch(req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
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
func (h *CommandHandlerGrpc[S]) Replay(ctx context.Context, req *pb.ReplayRequest) (*pb.ReplayResponse, error) {
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

// RegisterCommandHandlerGrpc returns a ServiceRegistrar that registers a command handler.
func RegisterCommandHandlerGrpc[S any](router *CommandRouter[S]) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterCommandHandlerServiceServer(server, NewCommandHandlerGrpc(router))
	}
}

// RunCommandHandlerServer starts a gRPC server for a command handler.
//
// Parameters:
//   - domain: The command handler's domain name
//   - defaultPort: Default TCP port if PORT env not set
//   - router: CommandRouter with registered handlers
func RunCommandHandlerServer[S any](domain, defaultPort string, router *CommandRouter[S]) {
	RunServer(RegisterCommandHandlerGrpc(router), ServerOptions{
		ServiceName:      "CommandHandler",
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

// Handle processes source events and returns commands for other aggregates.
// Sagas are stateless translators - they receive source events only.
func (h *SagaHandler) Handle(ctx context.Context, req *pb.SagaHandleRequest) (*pb.SagaResponse, error) {
	commands, err := h.router.Dispatch(req.Source, nil)
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

// ============================================================================
// Trait-Based Handlers
// ============================================================================
//
// These handlers use the trait-based CommandHandlerRouter and SagaRouter
// which delegate to CommandHandlerDomainHandler and SagaDomainHandler interfaces.

// TraitCommandHandlerGrpc wraps a CommandHandlerRouter for the gRPC CommandHandler service.
//
// Maps domain errors to gRPC status codes:
//   - CommandRejectedError -> FAILED_PRECONDITION
//   - Other errors -> INVALID_ARGUMENT
type TraitCommandHandlerGrpc[S any] struct {
	pb.UnimplementedCommandHandlerServiceServer
	router      *CommandHandlerRouter[S]
	statePacker StatePacker[S]
}

// NewTraitCommandHandlerGrpc creates a new command handler with the given router.
func NewTraitCommandHandlerGrpc[S any](router *CommandHandlerRouter[S]) *TraitCommandHandlerGrpc[S] {
	return &TraitCommandHandlerGrpc[S]{router: router}
}

// WithReplay enables Replay RPC support by providing a state packer.
func (h *TraitCommandHandlerGrpc[S]) WithReplay(packer StatePacker[S]) *TraitCommandHandlerGrpc[S] {
	h.statePacker = packer
	return h
}

// Handle processes a contextual command asynchronously.
func (h *TraitCommandHandlerGrpc[S]) Handle(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

// HandleSync processes a contextual command synchronously.
func (h *TraitCommandHandlerGrpc[S]) HandleSync(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

func (h *TraitCommandHandlerGrpc[S]) dispatch(req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
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
func (h *TraitCommandHandlerGrpc[S]) Replay(ctx context.Context, req *pb.ReplayRequest) (*pb.ReplayResponse, error) {
	if h.statePacker == nil {
		return nil, status.Error(codes.Unimplemented,
			"Replay not implemented. Call WithReplay() to enable for MERGE_COMMUTATIVE strategy.")
	}

	eventBook := &pb.EventBook{
		Pages:    req.Events,
		Snapshot: req.BaseSnapshot,
	}

	state := h.router.RebuildState(eventBook)
	stateAny, err := h.statePacker(state)
	if err != nil {
		return nil, status.Error(codes.Internal, err.Error())
	}

	return &pb.ReplayResponse{State: stateAny}, nil
}

// RegisterTraitCommandHandlerGrpc returns a ServiceRegistrar that registers a command handler.
func RegisterTraitCommandHandlerGrpc[S any](router *CommandHandlerRouter[S]) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterCommandHandlerServiceServer(server, NewTraitCommandHandlerGrpc(router))
	}
}

// RunTraitCommandHandlerServer starts a gRPC server for a command handler using trait-based router.
func RunTraitCommandHandlerServer[S any](domain, defaultPort string, router *CommandHandlerRouter[S]) {
	RunServer(RegisterTraitCommandHandlerGrpc(router), ServerOptions{
		ServiceName:      "CommandHandler",
		Domain:           domain,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// TraitSagaHandler wraps a SagaRouter for the gRPC Saga service.
type TraitSagaHandler struct {
	pb.UnimplementedSagaServiceServer
	router *SagaRouter
}

// NewTraitSagaHandler creates a new saga handler with the given router.
func NewTraitSagaHandler(router *SagaRouter) *TraitSagaHandler {
	return &TraitSagaHandler{router: router}
}

// Handle processes source events and returns commands for other aggregates.
// Sagas are stateless translators - they receive source events only.
func (h *TraitSagaHandler) Handle(ctx context.Context, req *pb.SagaHandleRequest) (*pb.SagaResponse, error) {
	resp, err := h.router.Dispatch(req.Source, nil)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return resp, nil
}

// RegisterTraitSagaHandler returns a ServiceRegistrar that registers a saga handler.
func RegisterTraitSagaHandler(router *SagaRouter) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterSagaServiceServer(server, NewTraitSagaHandler(router))
	}
}

// RunTraitSagaServer starts a gRPC server for a saga using trait-based router.
func RunTraitSagaServer(name, defaultPort string, router *SagaRouter) {
	RunServer(RegisterTraitSagaHandler(router), ServerOptions{
		ServiceName:      "Saga",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// TraitProcessManagerHandler wraps a ProcessManagerRouter for the gRPC ProcessManager service.
type TraitProcessManagerHandler[S any] struct {
	pb.UnimplementedProcessManagerServiceServer
	router *ProcessManagerRouter[S]
}

// NewTraitProcessManagerHandler creates a new process manager handler.
func NewTraitProcessManagerHandler[S any](router *ProcessManagerRouter[S]) *TraitProcessManagerHandler[S] {
	return &TraitProcessManagerHandler[S]{router: router}
}

// Prepare declares which additional destinations are needed.
func (h *TraitProcessManagerHandler[S]) Prepare(ctx context.Context, req *pb.ProcessManagerPrepareRequest) (*pb.ProcessManagerPrepareResponse, error) {
	destinations := h.router.PrepareDestinations(req.Trigger, req.ProcessState)
	return &pb.ProcessManagerPrepareResponse{Destinations: destinations}, nil
}

// Handle processes events and returns commands and process events.
func (h *TraitProcessManagerHandler[S]) Handle(ctx context.Context, req *pb.ProcessManagerHandleRequest) (*pb.ProcessManagerHandleResponse, error) {
	resp, err := h.router.Dispatch(req.Trigger, req.ProcessState, req.Destinations)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return resp, nil
}

// RegisterTraitProcessManagerHandler returns a ServiceRegistrar that registers a process manager handler.
func RegisterTraitProcessManagerHandler[S any](router *ProcessManagerRouter[S]) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterProcessManagerServiceServer(server, NewTraitProcessManagerHandler(router))
	}
}

// RunTraitProcessManagerServer starts a gRPC server for a process manager using trait-based router.
func RunTraitProcessManagerServer[S any](name, defaultPort string, router *ProcessManagerRouter[S]) {
	RunServer(RegisterTraitProcessManagerHandler(router), ServerOptions{
		ServiceName:      "ProcessManager",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// TraitProjectorHandler wraps a ProjectorRouter for the gRPC Projector service.
type TraitProjectorHandler struct {
	pb.UnimplementedProjectorServiceServer
	router *ProjectorRouter
}

// NewTraitProjectorHandler creates a new projector handler.
func NewTraitProjectorHandler(router *ProjectorRouter) *TraitProjectorHandler {
	return &TraitProjectorHandler{router: router}
}

// Handle processes an EventBook and returns a Projection.
func (h *TraitProjectorHandler) Handle(ctx context.Context, req *pb.EventBook) (*pb.Projection, error) {
	return h.router.Dispatch(req)
}

// HandleSpeculative processes events without side effects.
func (h *TraitProjectorHandler) HandleSpeculative(ctx context.Context, req *pb.EventBook) (*pb.Projection, error) {
	return h.Handle(ctx, req)
}

// RegisterTraitProjectorHandler returns a ServiceRegistrar that registers a projector handler.
func RegisterTraitProjectorHandler(router *ProjectorRouter) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterProjectorServiceServer(server, NewTraitProjectorHandler(router))
	}
}

// RunTraitProjectorServer starts a gRPC server for a projector using trait-based router.
func RunTraitProjectorServer(name, defaultPort string, router *ProjectorRouter) {
	RunServer(RegisterTraitProjectorHandler(router), ServerOptions{
		ServiceName:      "Projector",
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
	name         string
	prepareFn    PMPrepareFunc
	handleFn     PMHandleFunc
	revocationFn PMRevocationFunc
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

// OOCommandHandler interface for OO-style command handlers.
// Implemented by types that embed CommandHandlerBase.
type OOCommandHandler[S any] interface {
	Domain() string
	Handle(request *pb.ContextualCommand) (*pb.BusinessResponse, error)
	HandlerTypes() []string
}

// OOCommandHandlerFactory creates a new OO command handler instance with prior events.
type OOCommandHandlerFactory[S any, A OOCommandHandler[S]] func(events *pb.EventBook) A

// OOCommandHandlerGrpc wraps an OO-style command handler for the gRPC CommandHandler service.
//
// Unlike the functional CommandHandlerGrpc, this creates a new command handler instance
// for each request, passing in the prior events for state reconstruction.
//
// Example:
//
//	factory := func(events *pb.EventBook) *Table {
//	    return NewTable(events)
//	}
//	handler := NewOOCommandHandlerGrpc("table", factory)
type OOCommandHandlerGrpc[S any, A OOCommandHandler[S]] struct {
	pb.UnimplementedCommandHandlerServiceServer
	domain  string
	factory OOCommandHandlerFactory[S, A]
}

// NewOOCommandHandlerGrpc creates a new OO command handler.
//
// Parameters:
//   - domain: The command handler's domain name
//   - factory: Function to create a new command handler with prior events
func NewOOCommandHandlerGrpc[S any, A OOCommandHandler[S]](domain string, factory OOCommandHandlerFactory[S, A]) *OOCommandHandlerGrpc[S, A] {
	return &OOCommandHandlerGrpc[S, A]{
		domain:  domain,
		factory: factory,
	}
}

// Handle processes a contextual command asynchronously.
func (h *OOCommandHandlerGrpc[S, A]) Handle(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

// HandleSync processes a contextual command synchronously.
func (h *OOCommandHandlerGrpc[S, A]) HandleSync(ctx context.Context, req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	return h.dispatch(req)
}

func (h *OOCommandHandlerGrpc[S, A]) dispatch(req *pb.ContextualCommand) (*pb.BusinessResponse, error) {
	// Create command handler with prior events
	ch := h.factory(req.Events)

	// Dispatch command
	resp, err := ch.Handle(req)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	return resp, nil
}

// RegisterOOCommandHandlerGrpc returns a ServiceRegistrar that registers an OO command handler.
func RegisterOOCommandHandlerGrpc[S any, A OOCommandHandler[S]](domain string, factory OOCommandHandlerFactory[S, A]) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterCommandHandlerServiceServer(server, NewOOCommandHandlerGrpc(domain, factory))
	}
}

// RunOOCommandHandlerServer starts a gRPC server for an OO-style command handler.
//
// Parameters:
//   - domain: The command handler's domain name
//   - defaultPort: Default TCP port if PORT env not set
//   - factory: Function to create a new command handler with prior events
func RunOOCommandHandlerServer[S any, A OOCommandHandler[S]](domain, defaultPort string, factory OOCommandHandlerFactory[S, A]) {
	RunServer(RegisterOOCommandHandlerGrpc(domain, factory), ServerOptions{
		ServiceName:      "CommandHandler",
		Domain:           domain,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}

// OOSaga interface for OO-style sagas.
// Implemented by types that embed SagaBase.
// Sagas are stateless translators - they receive source events only.
type OOSaga interface {
	Name() string
	InputDomain() string
	OutputDomain() string
	Handle(source *pb.EventBook) (*SagaHandlerResponse, error)
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

// Handle processes source events and returns commands for other aggregates.
// Sagas are stateless translators - they receive source events only.
func (h *OOSagaHandler) Handle(ctx context.Context, req *pb.SagaHandleRequest) (*pb.SagaResponse, error) {
	response, err := h.saga.Handle(req.Source)
	if err != nil {
		var rejected CommandRejectedError
		if errors.As(err, &rejected) {
			return nil, status.Error(codes.FailedPrecondition, rejected.Message)
		}
		return nil, status.Error(codes.InvalidArgument, err.Error())
	}
	if response == nil {
		response = &SagaHandlerResponse{}
	}
	return &pb.SagaResponse{Commands: response.Commands, Events: response.Events}, nil
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

// ============================================================================
// Upcaster Handler
// ============================================================================

// UpcasterHandleFunc transforms a slice of EventPages to their current versions.
type UpcasterHandleFunc func(events []*pb.EventPage) []*pb.EventPage

// UpcasterGrpcHandler wraps a handle function for the gRPC Upcaster service.
//
// Example:
//
//	router := NewUpcasterRouter("player").
//	    On("examples.PlayerRegisteredV1", upcastPlayerRegistered)
//
//	handler := NewUpcasterGrpcHandler("upcaster-player", "player").
//	    WithHandle(func(events []*pb.EventPage) []*pb.EventPage {
//	        return router.Upcast(events)
//	    })
//
//	RunUpcasterServer("upcaster-player", "50401", handler)
type UpcasterGrpcHandler struct {
	pb.UnimplementedUpcasterServiceServer
	name     string
	domain   string
	handleFn UpcasterHandleFunc
}

// NewUpcasterGrpcHandler creates a new upcaster handler.
func NewUpcasterGrpcHandler(name, domain string) *UpcasterGrpcHandler {
	return &UpcasterGrpcHandler{
		name:   name,
		domain: domain,
	}
}

// WithHandle sets the event transformation callback.
func (h *UpcasterGrpcHandler) WithHandle(fn UpcasterHandleFunc) *UpcasterGrpcHandler {
	h.handleFn = fn
	return h
}

// Upcast transforms events to current versions.
func (h *UpcasterGrpcHandler) Upcast(ctx context.Context, req *pb.UpcastRequest) (*pb.UpcastResponse, error) {
	events := req.Events
	if h.handleFn != nil {
		events = h.handleFn(events)
	}
	return &pb.UpcastResponse{Events: events}, nil
}

// RegisterUpcasterGrpcHandler returns a ServiceRegistrar that registers an upcaster handler.
func RegisterUpcasterGrpcHandler(handler *UpcasterGrpcHandler) ServiceRegistrar {
	return func(server *grpc.Server) {
		pb.RegisterUpcasterServiceServer(server, handler)
	}
}

// RunUpcasterServer starts a gRPC server for an upcaster.
//
// Parameters:
//   - name: The upcaster's name (e.g., "upcaster-player")
//   - defaultPort: Default TCP port if PORT env not set
//   - handler: UpcasterGrpcHandler with configured handle function
func RunUpcasterServer(name, defaultPort string, handler *UpcasterGrpcHandler) {
	RunServer(RegisterUpcasterGrpcHandler(handler), ServerOptions{
		ServiceName:      "Upcaster",
		Domain:           name,
		DefaultPort:      defaultPort,
		EnableReflection: true,
	})
}
