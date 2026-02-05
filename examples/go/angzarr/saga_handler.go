package angzarr

import (
	"context"

	angzarrpb "angzarr/proto/angzarr"

	"google.golang.org/grpc"
)

// PrepareFunc examines source events and returns destination covers needed.
// Return nil for no action (saga ignores these events).
type PrepareFunc func(source *angzarrpb.EventBook) []*angzarrpb.Cover

// ExecuteFunc processes source events with destination state and returns commands.
type ExecuteFunc func(source *angzarrpb.EventBook, destinations []*angzarrpb.EventBook) []*angzarrpb.CommandBook

// SagaHandler implements the gRPC Saga service using an EventRouter.
//
// Simple mode (default): delegates Execute to router.Dispatch(source),
// Prepare returns empty destinations. Use for sagas that don't need
// destination aggregate state.
//
// Custom mode: provide WithPrepare/WithExecute overrides for sagas
// that need the two-phase protocol (e.g., saga-fulfillment needs
// destination state for sequence numbers).
type SagaHandler struct {
	angzarrpb.UnimplementedSagaServer
	router  *EventRouter
	prepare PrepareFunc
	execute ExecuteFunc
}

// NewSagaHandler creates a saga handler backed by an EventRouter.
func NewSagaHandler(router *EventRouter) *SagaHandler {
	return &SagaHandler{router: router}
}

// WithPrepare overrides the default (empty) prepare behavior.
func (h *SagaHandler) WithPrepare(fn PrepareFunc) *SagaHandler {
	h.prepare = fn
	return h
}

// WithExecute overrides the default (router dispatch) execute behavior.
func (h *SagaHandler) WithExecute(fn ExecuteFunc) *SagaHandler {
	h.execute = fn
	return h
}

// GetDescriptor returns the saga's component descriptor for service discovery.
func (h *SagaHandler) GetDescriptor(_ context.Context, _ *angzarrpb.GetDescriptorRequest) (*angzarrpb.ComponentDescriptor, error) {
	desc := h.router.Descriptor()
	inputs := make([]*angzarrpb.Target, len(desc.Inputs))
	for i, inp := range desc.Inputs {
		inputs[i] = &angzarrpb.Target{
			Domain: inp.Domain,
			Types:  inp.EventTypes,
		}
	}
	return &angzarrpb.ComponentDescriptor{
		Name:          desc.Name,
		ComponentType: ComponentSaga,
		Inputs:        inputs,
	}, nil
}

// Prepare declares which destination aggregates this saga needs.
// Default: returns empty destinations (no destination state needed).
func (h *SagaHandler) Prepare(_ context.Context, req *angzarrpb.SagaPrepareRequest) (*angzarrpb.SagaPrepareResponse, error) {
	if h.prepare != nil {
		return &angzarrpb.SagaPrepareResponse{
			Destinations: h.prepare(req.GetSource()),
		}, nil
	}
	return &angzarrpb.SagaPrepareResponse{}, nil
}

// Execute produces commands given source events and destination state.
// Default: delegates to router.Dispatch(source), ignoring destinations.
func (h *SagaHandler) Execute(_ context.Context, req *angzarrpb.SagaExecuteRequest) (*angzarrpb.SagaResponse, error) {
	commands := h.executeCommands(req.GetSource(), req.GetDestinations())
	return &angzarrpb.SagaResponse{Commands: commands}, nil
}

func (h *SagaHandler) executeCommands(source *angzarrpb.EventBook, destinations []*angzarrpb.EventBook) []*angzarrpb.CommandBook {
	if h.execute != nil {
		return h.execute(source, destinations)
	}
	return h.router.Dispatch(source)
}

// Descriptor returns the saga's component descriptor.
func (h *SagaHandler) Descriptor() Descriptor {
	return h.router.Descriptor()
}

// RunSagaServer starts a gRPC server for a saga.
func RunSagaServer(cfg ServerConfig, handler *SagaHandler) error {
	return RunServer(cfg, func(s *grpc.Server) {
		angzarrpb.RegisterSagaServer(s, handler)
	})
}
