package logic

import (
	"encoding/json"
	"testing"

	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
)

const testCorrelationID = "corr-1"

func makeEventBook(domain string, event *anypb.Any, correlationID string) *angzarrpb.EventBook {
	root := uuid.NewSHA1(uuid.NameSpaceOID, []byte("order-123"))
	return &angzarrpb.EventBook{
		Cover: &angzarrpb.Cover{
			Domain:        domain,
			Root:          &angzarrpb.UUID{Value: root[:]},
			CorrelationId: correlationID,
		},
		Pages: []*angzarrpb.EventPage{
			{
				Sequence: &angzarrpb.EventPage_Num{Num: 0},
				Event:    event,
			},
		},
	}
}

func paymentEvent(t *testing.T) *anypb.Any {
	t.Helper()
	a, err := anypb.New(&examples.PaymentSubmitted{
		PaymentMethod: "card",
		AmountCents:   5000,
	})
	if err != nil {
		t.Fatalf("failed to marshal PaymentSubmitted: %v", err)
	}
	return a
}

func stockEvent(t *testing.T) *anypb.Any {
	t.Helper()
	a, err := anypb.New(&examples.StockReserved{
		Quantity:     1,
		OrderId:      "order-123",
		NewAvailable: 9,
		NewReserved:  1,
		NewOnHand:    10,
	})
	if err != nil {
		t.Fatalf("failed to marshal StockReserved: %v", err)
	}
	return a
}

func packedEvent(t *testing.T) *anypb.Any {
	t.Helper()
	a, err := anypb.New(&examples.ItemsPacked{
		PackerId: "packer-1",
	})
	if err != nil {
		t.Fatalf("failed to marshal ItemsPacked: %v", err)
	}
	return a
}

// mergePMStates combines two PM event books (simulates persisted state accumulation).
func mergePMStates(s1, s2 *angzarrpb.EventBook) *angzarrpb.EventBook {
	if s1 == nil {
		return s2
	}
	if s2 == nil {
		return s1
	}
	merged := &angzarrpb.EventBook{
		Cover: s1.Cover,
		Pages: append(s1.Pages, s2.Pages...),
	}
	return merged
}

func TestFirstEventNoDispatch(t *testing.T) {
	trigger := makeEventBook("order", paymentEvent(t), testCorrelationID)

	commands, pmEvents := Handle(trigger, nil, nil)

	if len(commands) != 0 {
		t.Fatalf("expected no commands on first event, got %d", len(commands))
	}
	if pmEvents == nil {
		t.Fatal("expected PM events, got nil")
	}
	if len(pmEvents.Pages) != 1 {
		t.Fatalf("expected 1 PM event page, got %d", len(pmEvents.Pages))
	}

	// Verify it recorded the correct prerequisite.
	var evt prerequisiteCompleted
	if err := json.Unmarshal(pmEvents.Pages[0].Event.Value, &evt); err != nil {
		t.Fatalf("failed to unmarshal prerequisite event: %v", err)
	}
	if evt.Prerequisite != prereqPayment {
		t.Errorf("expected prerequisite %q, got %q", prereqPayment, evt.Prerequisite)
	}
}

func TestSecondEventNoDispatch(t *testing.T) {
	trigger1 := makeEventBook("order", paymentEvent(t), testCorrelationID)
	_, pmState1 := Handle(trigger1, nil, nil)

	trigger2 := makeEventBook("inventory", stockEvent(t), testCorrelationID)
	commands, pmEvents := Handle(trigger2, pmState1, nil)

	if len(commands) != 0 {
		t.Fatalf("expected no commands on second event, got %d", len(commands))
	}
	if pmEvents == nil {
		t.Fatal("expected PM events, got nil")
	}
}

func TestThirdEventTriggersDispatch(t *testing.T) {
	trigger1 := makeEventBook("order", paymentEvent(t), testCorrelationID)
	_, pmState1 := Handle(trigger1, nil, nil)

	trigger2 := makeEventBook("inventory", stockEvent(t), testCorrelationID)
	_, pmState2 := Handle(trigger2, pmState1, nil)

	mergedState := mergePMStates(pmState1, pmState2)

	trigger3 := makeEventBook("fulfillment", packedEvent(t), testCorrelationID)
	commands, pmEvents := Handle(trigger3, mergedState, nil)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command (Ship), got %d", len(commands))
	}
	if commands[0].Cover.Domain != fulfillmentDomain {
		t.Errorf("expected command domain %q, got %q", fulfillmentDomain, commands[0].Cover.Domain)
	}

	// Verify the Ship command is present.
	var ship examples.Ship
	if err := commands[0].Pages[0].Command.UnmarshalTo(&ship); err != nil {
		t.Fatalf("failed to unmarshal Ship command: %v", err)
	}
	if ship.Carrier == "" {
		t.Error("expected non-empty carrier")
	}

	if pmEvents == nil {
		t.Fatal("expected PM events, got nil")
	}
	if len(pmEvents.Pages) != 2 {
		t.Fatalf("expected 2 PM event pages (PrerequisiteCompleted + DispatchIssued), got %d", len(pmEvents.Pages))
	}

	// First page: PrerequisiteCompleted
	if pmEvents.Pages[0].Event.TypeUrl != typeURLPrerequisiteCompleted {
		t.Errorf("expected first event type %q, got %q", typeURLPrerequisiteCompleted, pmEvents.Pages[0].Event.TypeUrl)
	}
	// Second page: DispatchIssued
	if pmEvents.Pages[1].Event.TypeUrl != typeURLDispatchIssued {
		t.Errorf("expected second event type %q, got %q", typeURLDispatchIssued, pmEvents.Pages[1].Event.TypeUrl)
	}
}

func TestIdempotentAfterDispatch(t *testing.T) {
	// Build state that includes DispatchIssued.
	root := uuid.NewSHA1(uuid.NameSpaceOID, []byte("order-123"))
	prereqBytes, _ := json.Marshal(prerequisiteCompleted{
		Prerequisite: prereqPayment,
		Completed:    []string{prereqPayment},
		Remaining:    []string{prereqInventory, prereqFulfillment},
	})
	dispatchBytes, _ := json.Marshal(dispatchIssued{
		Completed: []string{prereqPayment, prereqInventory, prereqFulfillment},
	})

	dispatchedState := &angzarrpb.EventBook{
		Cover: &angzarrpb.Cover{
			Domain:        PMDomain,
			Root:          &angzarrpb.UUID{Value: root[:]},
			CorrelationId: testCorrelationID,
		},
		Pages: []*angzarrpb.EventPage{
			{
				Sequence: &angzarrpb.EventPage_Num{Num: 0},
				Event: &anypb.Any{
					TypeUrl: typeURLPrerequisiteCompleted,
					Value:   prereqBytes,
				},
			},
			{
				Sequence: &angzarrpb.EventPage_Num{Num: 1},
				Event: &anypb.Any{
					TypeUrl: typeURLDispatchIssued,
					Value:   dispatchBytes,
				},
			},
		},
	}

	trigger := makeEventBook("order", paymentEvent(t), testCorrelationID)
	commands, pmEvents := Handle(trigger, dispatchedState, nil)

	if len(commands) != 0 {
		t.Fatalf("expected no commands after dispatch, got %d", len(commands))
	}
	if pmEvents != nil {
		t.Fatalf("expected no PM events after dispatch, got %v", pmEvents)
	}
}

func TestNoCorrelationIDSkips(t *testing.T) {
	trigger := makeEventBook("order", paymentEvent(t), "")

	commands, pmEvents := Handle(trigger, nil, nil)

	if len(commands) != 0 {
		t.Fatalf("expected no commands for empty correlation, got %d", len(commands))
	}
	if pmEvents != nil {
		t.Fatalf("expected nil PM events for empty correlation, got %v", pmEvents)
	}
}

func TestDuplicatePrerequisiteNoop(t *testing.T) {
	trigger1 := makeEventBook("order", paymentEvent(t), testCorrelationID)
	_, pmState1 := Handle(trigger1, nil, nil)

	if pmState1 == nil {
		t.Fatal("expected PM state after first event")
	}

	// Send the same event again with the PM state from the first call.
	trigger2 := makeEventBook("order", paymentEvent(t), testCorrelationID)
	commands, pmEvents := Handle(trigger2, pmState1, nil)

	if len(commands) != 0 {
		t.Fatalf("expected no commands for duplicate prerequisite, got %d", len(commands))
	}
	if pmEvents != nil {
		t.Fatalf("expected nil PM events for duplicate prerequisite, got %v", pmEvents)
	}
}

func TestPMRootDeterministicFromCorrelationID(t *testing.T) {
	trigger := makeEventBook("order", paymentEvent(t), "corr-abc")
	_, pmEvents1 := Handle(trigger, nil, nil)
	_, pmEvents2 := Handle(trigger, nil, nil)

	if pmEvents1 == nil || pmEvents2 == nil {
		t.Fatal("expected PM events from both calls")
	}

	root1 := pmEvents1.Cover.Root.Value
	root2 := pmEvents2.Cover.Root.Value

	expectedRoot := uuid.NewSHA1(uuid.NameSpaceOID, []byte("corr-abc"))
	for i := range root1 {
		if root1[i] != root2[i] {
			t.Fatal("PM root should be deterministic for same correlation ID")
		}
		if root1[i] != expectedRoot[i] {
			t.Fatalf("PM root mismatch at byte %d: expected %x, got %x", i, expectedRoot[i], root1[i])
		}
	}
}

func TestShipCommandContainsOrderID(t *testing.T) {
	trigger1 := makeEventBook("order", paymentEvent(t), testCorrelationID)
	_, pmState1 := Handle(trigger1, nil, nil)

	trigger2 := makeEventBook("inventory", stockEvent(t), testCorrelationID)
	_, pmState2 := Handle(trigger2, pmState1, nil)

	mergedState := mergePMStates(pmState1, pmState2)

	trigger3 := makeEventBook("fulfillment", packedEvent(t), testCorrelationID)
	commands, _ := Handle(trigger3, mergedState, nil)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}

	var ship examples.Ship
	if err := commands[0].Pages[0].Command.UnmarshalTo(&ship); err != nil {
		t.Fatalf("failed to unmarshal Ship: %v", err)
	}

	// Carrier should be "auto-<uuid>" derived from the trigger's root.
	if len(ship.Carrier) < 5 {
		t.Errorf("expected carrier with 'auto-' prefix, got %q", ship.Carrier)
	}
}

func TestCommandCorrelationIDPassedThrough(t *testing.T) {
	trigger1 := makeEventBook("order", paymentEvent(t), "my-corr-id")
	_, pmState1 := Handle(trigger1, nil, nil)

	trigger2 := makeEventBook("inventory", stockEvent(t), "my-corr-id")
	_, pmState2 := Handle(trigger2, pmState1, nil)

	mergedState := mergePMStates(pmState1, pmState2)

	trigger3 := makeEventBook("fulfillment", packedEvent(t), "my-corr-id")
	commands, _ := Handle(trigger3, mergedState, nil)

	if len(commands) != 1 {
		t.Fatalf("expected 1 command, got %d", len(commands))
	}
	if commands[0].Cover.CorrelationId != "my-corr-id" {
		t.Errorf("expected correlation_id %q, got %q", "my-corr-id", commands[0].Cover.CorrelationId)
	}
}
