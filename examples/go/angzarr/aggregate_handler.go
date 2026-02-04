package angzarr

import (
	"context"

	angzarrpb "angzarr/proto/angzarr"

	"google.golang.org/grpc"
)

// AggregateHandler implements the gRPC Aggregate service using a CommandRouter.
//
// Replaces per-service server structs and manual dispatch. Just create a
// CommandRouter, register handlers, and pass to RunAggregateServer.
//
// Example:
//
//	router := angzarr.NewCommandRouter("customer", rebuildState).
//	    On("CreateCustomer", handleCreateCustomer).
//	    On("AddLoyaltyPoints", handleAddLoyaltyPoints)
//
//	cfg := angzarr.ServerConfig{Domain: "customer", DefaultPort: "50052"}
//	angzarr.RunAggregateServer(cfg, router)
type AggregateHandler[S any] struct {
	angzarrpb.UnimplementedAggregateServer
	router *CommandRouter[S]
}

// NewAggregateHandler creates an Aggregate gRPC handler backed by a CommandRouter.
func NewAggregateHandler[S any](router *CommandRouter[S]) *AggregateHandler[S] {
	return &AggregateHandler[S]{router: router}
}

// GetDescriptor returns the aggregate's component descriptor for service discovery.
func (h *AggregateHandler[S]) GetDescriptor(_ context.Context, _ *angzarrpb.GetDescriptorRequest) (*angzarrpb.ComponentDescriptor, error) {
	desc := h.router.Descriptor()
	inputs := make([]*angzarrpb.Subscription, len(desc.Inputs))
	for i, inp := range desc.Inputs {
		inputs[i] = &angzarrpb.Subscription{
			Domain:     inp.Domain,
			EventTypes: inp.EventTypes,
		}
	}
	return &angzarrpb.ComponentDescriptor{
		Name:          desc.Name,
		ComponentType: ComponentAggregate,
		Inputs:        inputs,
	}, nil
}

// Handle dispatches a ContextualCommand through the router.
//
// Errors from the router (including CommandError) are mapped to gRPC status
// codes via MapCommandError.
func (h *AggregateHandler[S]) Handle(_ context.Context, req *angzarrpb.ContextualCommand) (*angzarrpb.BusinessResponse, error) {
	resp, err := h.router.Dispatch(req)
	if err != nil {
		return nil, MapCommandError(err)
	}
	return resp, nil
}

// Descriptor returns the component descriptor from the router.
func (h *AggregateHandler[S]) Descriptor() Descriptor {
	return h.router.Descriptor()
}

// RunAggregateServer creates and runs a gRPC server for an aggregate.
//
// Combines RunServer with a CommandRouter-backed handler.
func RunAggregateServer[S any](cfg ServerConfig, router *CommandRouter[S]) error {
	return RunServer(cfg, func(s *grpc.Server) {
		angzarrpb.RegisterAggregateServer(s, NewAggregateHandler(router))
	})
}
