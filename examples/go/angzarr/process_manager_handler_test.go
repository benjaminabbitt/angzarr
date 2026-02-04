package angzarr

import (
	"context"
	"testing"

	angzarrpb "angzarr/proto/angzarr"
)

func TestPMHandler_GetDescriptor(t *testing.T) {
	h := NewProcessManagerHandler("order-fulfillment").
		ListenTo("order", "OrderCompleted").
		ListenTo("inventory", "StockReserved")

	resp, err := h.GetDescriptor(context.Background(), &angzarrpb.GetDescriptorRequest{})
	if err != nil {
		t.Fatal(err)
	}
	if resp.Name != "order-fulfillment" {
		t.Errorf("expected name 'order-fulfillment', got %q", resp.Name)
	}
	if resp.ComponentType != ComponentProcessManager {
		t.Errorf("expected type %q, got %q", ComponentProcessManager, resp.ComponentType)
	}
	if len(resp.Inputs) != 2 {
		t.Fatalf("expected 2 inputs, got %d", len(resp.Inputs))
	}
	if resp.Inputs[0].Domain != "order" {
		t.Errorf("expected domain 'order', got %q", resp.Inputs[0].Domain)
	}
	if resp.Inputs[1].Domain != "inventory" {
		t.Errorf("expected domain 'inventory', got %q", resp.Inputs[1].Domain)
	}
}

func TestPMHandler_PrepareDefault(t *testing.T) {
	h := NewProcessManagerHandler("test")

	resp, err := h.Prepare(context.Background(), &angzarrpb.ProcessManagerPrepareRequest{
		Trigger: &angzarrpb.EventBook{},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(resp.Destinations) != 0 {
		t.Errorf("expected empty destinations, got %d", len(resp.Destinations))
	}
}

func TestPMHandler_PrepareCustom(t *testing.T) {
	h := NewProcessManagerHandler("test").
		WithPrepare(func(trigger, processState *angzarrpb.EventBook) []*angzarrpb.Cover {
			return []*angzarrpb.Cover{
				{Domain: "fulfillment", Root: &angzarrpb.UUID{Value: []byte("root-1")}},
			}
		})

	resp, err := h.Prepare(context.Background(), &angzarrpb.ProcessManagerPrepareRequest{
		Trigger: &angzarrpb.EventBook{},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(resp.Destinations) != 1 {
		t.Fatalf("expected 1 destination, got %d", len(resp.Destinations))
	}
	if resp.Destinations[0].Domain != "fulfillment" {
		t.Errorf("expected domain 'fulfillment', got %q", resp.Destinations[0].Domain)
	}
}

func TestPMHandler_HandleDefault(t *testing.T) {
	h := NewProcessManagerHandler("test")

	resp, err := h.Handle(context.Background(), &angzarrpb.ProcessManagerHandleRequest{
		Trigger: &angzarrpb.EventBook{},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(resp.Commands) != 0 {
		t.Errorf("expected empty commands, got %d", len(resp.Commands))
	}
	if resp.ProcessEvents != nil {
		t.Errorf("expected nil process events, got %v", resp.ProcessEvents)
	}
}

func TestPMHandler_HandleCustom(t *testing.T) {
	h := NewProcessManagerHandler("test").
		WithHandle(func(trigger, processState *angzarrpb.EventBook, destinations []*angzarrpb.EventBook) ([]*angzarrpb.CommandBook, *angzarrpb.EventBook) {
			return []*angzarrpb.CommandBook{
					{Cover: &angzarrpb.Cover{Domain: "fulfillment"}},
				}, &angzarrpb.EventBook{
					Pages: []*angzarrpb.EventPage{{Sequence: &angzarrpb.EventPage_Num{Num: 0}}},
				}
		})

	resp, err := h.Handle(context.Background(), &angzarrpb.ProcessManagerHandleRequest{
		Trigger: &angzarrpb.EventBook{},
	})
	if err != nil {
		t.Fatal(err)
	}
	if len(resp.Commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(resp.Commands))
	}
	if resp.Commands[0].Cover.Domain != "fulfillment" {
		t.Errorf("expected domain 'fulfillment', got %q", resp.Commands[0].Cover.Domain)
	}
	if resp.ProcessEvents == nil || len(resp.ProcessEvents.Pages) != 1 {
		t.Error("expected process events with 1 page")
	}
}

func TestPMHandler_Descriptor(t *testing.T) {
	h := NewProcessManagerHandler("test-pm").
		ListenTo("order", "OrderCompleted", "OrderCancelled").
		ListenTo("fulfillment", "Shipped")

	desc := h.Descriptor()
	if desc.Name != "test-pm" {
		t.Errorf("expected name 'test-pm', got %q", desc.Name)
	}
	if desc.ComponentType != ComponentProcessManager {
		t.Errorf("expected type %q, got %q", ComponentProcessManager, desc.ComponentType)
	}
	if len(desc.Inputs) != 2 {
		t.Fatalf("expected 2 inputs, got %d", len(desc.Inputs))
	}
	if len(desc.Inputs[0].EventTypes) != 2 {
		t.Errorf("expected 2 event types for order, got %d", len(desc.Inputs[0].EventTypes))
	}
}
