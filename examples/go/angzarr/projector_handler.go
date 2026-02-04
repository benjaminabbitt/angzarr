package angzarr

import (
	"context"

	angzarrpb "angzarr/proto/angzarr"

	"google.golang.org/grpc"
)

// ProjectorHandleFunc processes an EventBook and returns a Projection.
type ProjectorHandleFunc func(book *angzarrpb.EventBook) (*angzarrpb.Projection, error)

// ProjectorHandler implements the gRPC Projector service.
//
// Projectors consume events and produce read models. The handler receives
// EventBooks and delegates to a user-provided function for projection logic.
type ProjectorHandler struct {
	angzarrpb.UnimplementedProjectorServer
	name     string
	domains  []string
	handleFn ProjectorHandleFunc
}

// NewProjectorHandler creates a projector handler with the given name and input domains.
func NewProjectorHandler(name string, domains ...string) *ProjectorHandler {
	return &ProjectorHandler{name: name, domains: domains}
}

// WithHandle sets the event handling callback.
func (h *ProjectorHandler) WithHandle(fn ProjectorHandleFunc) *ProjectorHandler {
	h.handleFn = fn
	return h
}

// GetDescriptor returns the component descriptor.
func (h *ProjectorHandler) GetDescriptor(_ context.Context, _ *angzarrpb.GetDescriptorRequest) (*angzarrpb.ComponentDescriptor, error) {
	desc := h.Descriptor()
	var inputs []*angzarrpb.Subscription
	for _, input := range desc.Inputs {
		inputs = append(inputs, &angzarrpb.Subscription{
			Domain:     input.Domain,
			EventTypes: input.EventTypes,
		})
	}
	return &angzarrpb.ComponentDescriptor{
		Name:          desc.Name,
		ComponentType: desc.ComponentType,
		Inputs:        inputs,
	}, nil
}

// Handle processes an EventBook and returns a Projection.
func (h *ProjectorHandler) Handle(_ context.Context, book *angzarrpb.EventBook) (*angzarrpb.Projection, error) {
	if h.handleFn != nil {
		return h.handleFn(book)
	}
	return &angzarrpb.Projection{}, nil
}

// Descriptor builds a component descriptor from registered domains.
func (h *ProjectorHandler) Descriptor() Descriptor {
	var inputs []SubscriptionDesc
	for _, domain := range h.domains {
		inputs = append(inputs, SubscriptionDesc{Domain: domain})
	}
	return Descriptor{
		Name:          h.name,
		ComponentType: ComponentProjector,
		Inputs:        inputs,
	}
}

// RunProjectorServer starts a gRPC server for a projector.
func RunProjectorServer(cfg ServerConfig, handler *ProjectorHandler) error {
	return RunServer(cfg, func(s *grpc.Server) {
		angzarrpb.RegisterProjectorServer(s, handler)
	})
}
