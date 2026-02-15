package angzarr

import (
	"context"
	"errors"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
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

// AggregateHandler wraps a CommandRouter for the gRPC Aggregate service.
//
// Maps domain errors to gRPC status codes:
//   - CommandRejectedError -> FAILED_PRECONDITION
//   - Other errors -> INVALID_ARGUMENT
type AggregateHandler[S any] struct {
	pb.UnimplementedAggregateServiceServer
	router *CommandRouter[S]
}

// NewAggregateHandler creates a new aggregate handler with the given router.
func NewAggregateHandler[S any](router *CommandRouter[S]) *AggregateHandler[S] {
	return &AggregateHandler[S]{router: router}
}

// GetDescriptor returns the component descriptor for service discovery.
func (h *AggregateHandler[S]) GetDescriptor(ctx context.Context, req *pb.GetDescriptorRequest) (*pb.ComponentDescriptor, error) {
	return h.router.Descriptor(), nil
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

// Descriptor returns the router's component descriptor.
func (h *AggregateHandler[S]) Descriptor() *pb.ComponentDescriptor {
	return h.router.Descriptor()
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

// GetDescriptor returns the component descriptor for service discovery.
func (h *SagaHandler) GetDescriptor(ctx context.Context, req *pb.GetDescriptorRequest) (*pb.ComponentDescriptor, error) {
	return h.router.Descriptor(), nil
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

// Descriptor returns the router's component descriptor.
func (h *SagaHandler) Descriptor() *pb.ComponentDescriptor {
	return h.router.Descriptor()
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

// GetDescriptor returns the component descriptor for service discovery.
func (h *ProjectorHandler) GetDescriptor(ctx context.Context, req *pb.GetDescriptorRequest) (*pb.ComponentDescriptor, error) {
	inputs := make([]*pb.Target, len(h.domains))
	for i, domain := range h.domains {
		inputs[i] = &pb.Target{Domain: domain}
	}
	return &pb.ComponentDescriptor{
		Name:          h.name,
		ComponentType: ComponentProjector,
		Inputs:        inputs,
	}, nil
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
	inputs        []*pb.Target
	prepareFn     PMPrepareFunc
	handleFn      PMHandleFunc
	revocationFn  PMRevocationFunc
}

// NewProcessManagerHandler creates a new process manager handler.
func NewProcessManagerHandler(name string) *ProcessManagerHandler {
	return &ProcessManagerHandler{
		name:   name,
		inputs: make([]*pb.Target, 0),
	}
}

// ListenTo subscribes to events from a domain.
func (h *ProcessManagerHandler) ListenTo(domain string, types ...string) *ProcessManagerHandler {
	h.inputs = append(h.inputs, &pb.Target{Domain: domain, Types: types})
	return h
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

// GetDescriptor returns the component descriptor for service discovery.
func (h *ProcessManagerHandler) GetDescriptor(ctx context.Context, req *pb.GetDescriptorRequest) (*pb.ComponentDescriptor, error) {
	return &pb.ComponentDescriptor{
		Name:          h.name,
		ComponentType: ComponentProcessManager,
		Inputs:        h.inputs,
	}, nil
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
