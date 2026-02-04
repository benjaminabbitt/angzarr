package angzarr

import (
	"context"
	"testing"

	angzarrpb "angzarr/proto/angzarr"

	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
)

// ============================================================================
// AggregateHandler tests
// ============================================================================

func TestAggregateHandlerDispatch(t *testing.T) {
	handler := NewAggregateHandler(
		NewCommandRouter(domainTest, dummyRebuild).
			On(suffixCommandA, handlerA),
	)

	cmd := makeContextualCommand(typeURLCommandA, nil)
	resp, err := handler.Handle(context.Background(), cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	events := resp.GetEvents()
	if events == nil || len(events.Pages) != 1 {
		t.Fatal("expected 1 event page")
	}
	if events.Pages[0].Event.TypeUrl != "handled_a:seq=0" {
		t.Errorf("expected 'handled_a:seq=0', got %q", events.Pages[0].Event.TypeUrl)
	}
}

func TestAggregateHandlerMapsFailedPrecondition(t *testing.T) {
	handler := NewAggregateHandler(
		NewCommandRouter(domainTest, dummyRebuild).
			On(suffixCommandA, func(_ *angzarrpb.CommandBook, _ []byte, _ *testState, _ uint32) (*angzarrpb.EventBook, error) {
				return nil, NewFailedPrecondition("entity already exists")
			}),
	)

	cmd := makeContextualCommand(typeURLCommandA, nil)
	_, err := handler.Handle(context.Background(), cmd)
	if err == nil {
		t.Fatal("expected error")
	}

	st, ok := status.FromError(err)
	if !ok {
		t.Fatal("expected gRPC status error")
	}
	if st.Code() != codes.FailedPrecondition {
		t.Errorf("expected FailedPrecondition, got %v", st.Code())
	}
	if st.Message() != "entity already exists" {
		t.Errorf("expected 'entity already exists', got %q", st.Message())
	}
}

func TestAggregateHandlerMapsInvalidArgument(t *testing.T) {
	handler := NewAggregateHandler(
		NewCommandRouter(domainTest, dummyRebuild).
			On(suffixCommandA, func(_ *angzarrpb.CommandBook, _ []byte, _ *testState, _ uint32) (*angzarrpb.EventBook, error) {
				return nil, NewInvalidArgument("name is required")
			}),
	)

	cmd := makeContextualCommand(typeURLCommandA, nil)
	_, err := handler.Handle(context.Background(), cmd)
	if err == nil {
		t.Fatal("expected error")
	}

	st, ok := status.FromError(err)
	if !ok {
		t.Fatal("expected gRPC status error")
	}
	if st.Code() != codes.InvalidArgument {
		t.Errorf("expected InvalidArgument, got %v", st.Code())
	}
}

func TestAggregateHandlerUnknownCommandMapsToInternal(t *testing.T) {
	handler := NewAggregateHandler(
		NewCommandRouter(domainTest, dummyRebuild).
			On(suffixCommandA, handlerA),
	)

	cmd := makeContextualCommand(typeURLUnknown, nil)
	_, err := handler.Handle(context.Background(), cmd)
	if err == nil {
		t.Fatal("expected error for unknown command")
	}

	st, ok := status.FromError(err)
	if !ok {
		t.Fatal("expected gRPC status error")
	}
	// Unknown command is a plain error (not CommandError), mapped as Internal
	if st.Code() != codes.Internal {
		t.Errorf("expected Internal, got %v", st.Code())
	}
}

func TestAggregateHandlerWithPriorEvents(t *testing.T) {
	handler := NewAggregateHandler(
		NewCommandRouter(domainTest, dummyRebuild).
			On(suffixCommandA, handlerA),
	)

	priorEvents := &angzarrpb.EventBook{
		Pages: []*angzarrpb.EventPage{{}, {}, {}},
	}
	cmd := makeContextualCommand(typeURLCommandA, priorEvents)

	resp, err := handler.Handle(context.Background(), cmd)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	events := resp.GetEvents()
	if events.Pages[0].Event.TypeUrl != "handled_a:seq=3" {
		t.Errorf("expected seq=3, got %q", events.Pages[0].Event.TypeUrl)
	}
}

func TestAggregateHandlerDescriptor(t *testing.T) {
	handler := NewAggregateHandler(
		NewCommandRouter(domainOrder, dummyRebuild).
			On(suffixCreateOrder, handlerA).
			On(suffixCancelOrder, handlerB),
	)

	desc := handler.Descriptor()
	if desc.Name != domainOrder {
		t.Errorf("expected name %q, got %q", domainOrder, desc.Name)
	}
	if desc.ComponentType != ComponentAggregate {
		t.Errorf("expected component_type %q, got %q", ComponentAggregate, desc.ComponentType)
	}
	if len(desc.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(desc.Inputs))
	}
	if len(desc.Inputs[0].EventTypes) != 2 {
		t.Fatalf("expected 2 event types, got %d", len(desc.Inputs[0].EventTypes))
	}
}
