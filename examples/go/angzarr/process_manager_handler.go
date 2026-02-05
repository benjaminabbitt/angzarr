package angzarr

import (
	"context"

	angzarrpb "angzarr/proto/angzarr"

	"google.golang.org/grpc"
)

// PMPrepareFunc examines the trigger and process state, returns additional destinations needed.
type PMPrepareFunc func(trigger *angzarrpb.EventBook, processState *angzarrpb.EventBook) []*angzarrpb.Cover

// PMHandleFunc processes trigger + process state + destinations, returns commands and process events.
type PMHandleFunc func(trigger *angzarrpb.EventBook, processState *angzarrpb.EventBook, destinations []*angzarrpb.EventBook) ([]*angzarrpb.CommandBook, *angzarrpb.EventBook)

// ProcessManagerHandler implements the gRPC ProcessManager service.
//
// Process managers are stateful coordinators for long-running workflows across
// multiple aggregates. They maintain their own event-sourced state and react to
// events from multiple domains.
//
// Two-phase protocol:
//   - Prepare: declare additional destinations needed beyond the trigger
//   - Handle: produce commands and process events given full context
type ProcessManagerHandler struct {
	angzarrpb.UnimplementedProcessManagerServer
	name      string
	inputs    []SubscriptionDesc
	prepareFn PMPrepareFunc
	handleFn  PMHandleFunc
}

// NewProcessManagerHandler creates a process manager handler with the given name.
func NewProcessManagerHandler(name string) *ProcessManagerHandler {
	return &ProcessManagerHandler{name: name}
}

// ListenTo subscribes to events from a domain.
func (h *ProcessManagerHandler) ListenTo(domain string, eventTypes ...string) *ProcessManagerHandler {
	h.inputs = append(h.inputs, SubscriptionDesc{Domain: domain, EventTypes: eventTypes})
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

// GetDescriptor returns the component descriptor.
func (h *ProcessManagerHandler) GetDescriptor(_ context.Context, _ *angzarrpb.GetDescriptorRequest) (*angzarrpb.ComponentDescriptor, error) {
	desc := h.Descriptor()
	var inputs []*angzarrpb.Target
	for _, input := range desc.Inputs {
		inputs = append(inputs, &angzarrpb.Target{
			Domain: input.Domain,
			Types:  input.EventTypes,
		})
	}
	return &angzarrpb.ComponentDescriptor{
		Name:          desc.Name,
		ComponentType: desc.ComponentType,
		Inputs:        inputs,
	}, nil
}

// Prepare declares additional destinations needed beyond the trigger.
func (h *ProcessManagerHandler) Prepare(_ context.Context, req *angzarrpb.ProcessManagerPrepareRequest) (*angzarrpb.ProcessManagerPrepareResponse, error) {
	if h.prepareFn != nil {
		destinations := h.prepareFn(req.GetTrigger(), req.GetProcessState())
		return &angzarrpb.ProcessManagerPrepareResponse{Destinations: destinations}, nil
	}
	return &angzarrpb.ProcessManagerPrepareResponse{}, nil
}

// Handle processes trigger + process state + destinations.
func (h *ProcessManagerHandler) Handle(_ context.Context, req *angzarrpb.ProcessManagerHandleRequest) (*angzarrpb.ProcessManagerHandleResponse, error) {
	if h.handleFn != nil {
		commands, events := h.handleFn(req.GetTrigger(), req.GetProcessState(), req.GetDestinations())
		return &angzarrpb.ProcessManagerHandleResponse{
			Commands:      commands,
			ProcessEvents: events,
		}, nil
	}
	return &angzarrpb.ProcessManagerHandleResponse{}, nil
}

// Descriptor builds a component descriptor from registered inputs.
func (h *ProcessManagerHandler) Descriptor() Descriptor {
	return Descriptor{
		Name:          h.name,
		ComponentType: ComponentProcessManager,
		Inputs:        h.inputs,
	}
}

// RunProcessManagerServer starts a gRPC server for a process manager.
func RunProcessManagerServer(cfg ServerConfig, handler *ProcessManagerHandler) error {
	return RunServer(cfg, func(s *grpc.Server) {
		angzarrpb.RegisterProcessManagerServer(s, handler)
	})
}
