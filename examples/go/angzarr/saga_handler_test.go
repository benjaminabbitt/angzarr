package angzarr

import (
	"context"
	"testing"

	"google.golang.org/protobuf/types/known/anypb"

	angzarrpb "angzarr/proto/angzarr"
)

// ============================================================================
// SagaHandler tests — simple mode (EventRouter dispatch)
// ============================================================================

func TestSagaHandlerPrepareDefaultEmpty(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		On(suffixOrderComplete, sagaHandler)
	handler := NewSagaHandler(router)

	resp, err := handler.Prepare(context.Background(), &angzarrpb.SagaPrepareRequest{
		Source: makeEventBook(typeURLOrderDone, corrID1, []byte{1, 2}),
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(resp.GetDestinations()) != 0 {
		t.Errorf("expected empty destinations, got %d", len(resp.GetDestinations()))
	}
}

func TestSagaHandlerExecuteDispatchesViaRouter(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		Output(domainFulfillment).
		On(suffixOrderComplete, sagaHandler)
	handler := NewSagaHandler(router)

	resp, err := handler.Execute(context.Background(), &angzarrpb.SagaExecuteRequest{
		Source: makeEventBook(typeURLOrderDone, corrID1, []byte{4, 5, 6}),
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(resp.Commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(resp.Commands))
	}
	if resp.Commands[0].Cover.Domain != domainFulfillment {
		t.Errorf("expected domain %q, got %q", domainFulfillment, resp.Commands[0].Cover.Domain)
	}
	if resp.Commands[0].Cover.CorrelationId != corrID1 {
		t.Errorf("expected correlation_id %q, got %q", corrID1, resp.Commands[0].Cover.CorrelationId)
	}
}

func TestSagaHandlerExecuteNoMatchReturnsEmpty(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		On(suffixOrderComplete, sagaHandler)
	handler := NewSagaHandler(router)

	resp, err := handler.Execute(context.Background(), &angzarrpb.SagaExecuteRequest{
		Source: makeEventBook(typeURLOtherEvent, corrID1, []byte{1}),
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(resp.Commands) != 0 {
		t.Errorf("expected 0 commands, got %d", len(resp.Commands))
	}
}

// ============================================================================
// SagaHandler tests — custom mode (WithPrepare/WithExecute)
// ============================================================================

func TestSagaHandlerCustomPrepare(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		On(suffixOrderComplete, sagaHandler)

	prepareFn := func(source *angzarrpb.EventBook) []*angzarrpb.Cover {
		if source == nil {
			return nil
		}
		return []*angzarrpb.Cover{
			{Domain: domainFulfillment, Root: source.GetCover().GetRoot()},
		}
	}

	handler := NewSagaHandler(router).WithPrepare(prepareFn)

	source := makeEventBook(typeURLOrderDone, corrID1, []byte{10, 20})
	resp, err := handler.Prepare(context.Background(), &angzarrpb.SagaPrepareRequest{
		Source: source,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(resp.GetDestinations()) != 1 {
		t.Fatalf("expected 1 destination, got %d", len(resp.GetDestinations()))
	}
	if resp.GetDestinations()[0].Domain != domainFulfillment {
		t.Errorf("expected domain %q, got %q", domainFulfillment, resp.GetDestinations()[0].Domain)
	}
	if string(resp.GetDestinations()[0].Root.Value) != string([]byte{10, 20}) {
		t.Error("expected root to match source root")
	}
}

func TestSagaHandlerCustomExecute(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		On(suffixOrderComplete, sagaHandler)

	executeFn := func(source *angzarrpb.EventBook, destinations []*angzarrpb.EventBook) []*angzarrpb.CommandBook {
		var seq uint32
		if len(destinations) > 0 && destinations[0] != nil {
			seq = uint32(len(destinations[0].Pages))
		}
		return []*angzarrpb.CommandBook{
			{
				Cover: &angzarrpb.Cover{Domain: domainFulfillment},
				Pages: []*angzarrpb.CommandPage{
					{Sequence: seq, Command: &anypb.Any{TypeUrl: "custom_cmd"}},
				},
			},
		}
	}

	handler := NewSagaHandler(router).WithExecute(executeFn)

	destEvents := &angzarrpb.EventBook{
		Pages: []*angzarrpb.EventPage{{}, {}, {}},
	}
	resp, err := handler.Execute(context.Background(), &angzarrpb.SagaExecuteRequest{
		Source:       makeEventBook(typeURLOrderDone, corrID1, []byte{1}),
		Destinations: []*angzarrpb.EventBook{destEvents},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(resp.Commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(resp.Commands))
	}
	if resp.Commands[0].Pages[0].Sequence != 3 {
		t.Errorf("expected sequence 3 (from 3 dest pages), got %d", resp.Commands[0].Pages[0].Sequence)
	}
}

func TestSagaHandlerDescriptor(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		Output(domainFulfillment).
		On(suffixOrderComplete, sagaHandler).
		On(suffixOrderCancel, sagaHandler)

	handler := NewSagaHandler(router)
	desc := handler.Descriptor()

	if desc.Name != sagaFulfillment {
		t.Errorf("expected name %q, got %q", sagaFulfillment, desc.Name)
	}
	if desc.ComponentType != ComponentSaga {
		t.Errorf("expected component_type %q, got %q", ComponentSaga, desc.ComponentType)
	}
	if len(desc.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(desc.Inputs))
	}
	if len(desc.Inputs[0].EventTypes) != 2 {
		t.Fatalf("expected 2 event types, got %d", len(desc.Inputs[0].EventTypes))
	}
}

func TestSagaHandlerGetDescriptorGRPC(t *testing.T) {
	router := NewEventRouter(sagaFulfillment, domainOrder).
		Output(domainFulfillment).
		On(suffixOrderComplete, sagaHandler)

	handler := NewSagaHandler(router)
	resp, err := handler.GetDescriptor(context.Background(), &angzarrpb.GetDescriptorRequest{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Name != sagaFulfillment {
		t.Errorf("expected name %q, got %q", sagaFulfillment, resp.Name)
	}
	if resp.ComponentType != ComponentSaga {
		t.Errorf("expected component_type %q, got %q", ComponentSaga, resp.ComponentType)
	}
	if len(resp.Inputs) != 1 {
		t.Fatalf("expected 1 input, got %d", len(resp.Inputs))
	}
	if resp.Inputs[0].Domain != domainOrder {
		t.Errorf("expected domain %q, got %q", domainOrder, resp.Inputs[0].Domain)
	}
}
