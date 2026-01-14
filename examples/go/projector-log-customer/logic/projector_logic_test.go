package logic

import (
	"testing"

	"projector-log-customer/proto/angzarr"
	"projector-log-customer/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

func TestProcessEventBook_NilEventBook(t *testing.T) {
	logic := NewProjectorLogic()
	results := logic.ProcessEventBook(nil)

	if results != nil {
		t.Errorf("expected nil results for nil event book, got %v", results)
	}
}

func TestProcessEventBook_EmptyEventBook(t *testing.T) {
	logic := NewProjectorLogic()
	results := logic.ProcessEventBook(&angzarr.EventBook{})

	if results != nil {
		t.Errorf("expected nil results for empty event book, got %v", results)
	}
}

func TestProcessEventBook_CustomerCreatedEvent(t *testing.T) {
	logic := NewProjectorLogic()

	event := &examples.CustomerCreated{
		Name:      "Alice",
		Email:     "alice@example.com",
		CreatedAt: timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "customer",
		},
		Pages: []*angzarr.EventPage{
			{
				Sequence: &angzarr.EventPage_Num{Num: 0},
				Event:    eventAny,
			},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}

	result := results[0]
	if result.Domain != "customer" {
		t.Errorf("expected domain 'customer', got %q", result.Domain)
	}
	if result.EventType != "CustomerCreated" {
		t.Errorf("expected event type 'CustomerCreated', got %q", result.EventType)
	}
	if result.Unknown {
		t.Error("expected known event type")
	}
	if result.Fields["name"] != "Alice" {
		t.Errorf("expected name 'Alice', got %v", result.Fields["name"])
	}
	if result.Fields["email"] != "alice@example.com" {
		t.Errorf("expected email 'alice@example.com', got %v", result.Fields["email"])
	}
	if _, ok := result.Fields["created_at"]; !ok {
		t.Error("expected created_at field")
	}
}

func TestProcessEventBook_LoyaltyPointsAddedEvent(t *testing.T) {
	logic := NewProjectorLogic()

	event := &examples.LoyaltyPointsAdded{
		Points:     100,
		NewBalance: 100,
		Reason:     "purchase",
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{
				Sequence: &angzarr.EventPage_Num{Num: 1},
				Event:    eventAny,
			},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}

	result := results[0]
	if result.EventType != "LoyaltyPointsAdded" {
		t.Errorf("expected event type 'LoyaltyPointsAdded', got %q", result.EventType)
	}
	if result.Unknown {
		t.Error("expected known event type")
	}
	if result.Fields["points"] != int32(100) {
		t.Errorf("expected points 100, got %v", result.Fields["points"])
	}
	if result.Fields["new_balance"] != int32(100) {
		t.Errorf("expected new_balance 100, got %v", result.Fields["new_balance"])
	}
	if result.Fields["reason"] != "purchase" {
		t.Errorf("expected reason 'purchase', got %v", result.Fields["reason"])
	}
}

func TestProcessEventBook_LoyaltyPointsRedeemedEvent(t *testing.T) {
	logic := NewProjectorLogic()

	event := &examples.LoyaltyPointsRedeemed{
		Points:         50,
		NewBalance:     50,
		RedemptionType: "discount",
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{
				Sequence: &angzarr.EventPage_Num{Num: 2},
				Event:    eventAny,
			},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}

	result := results[0]
	if result.EventType != "LoyaltyPointsRedeemed" {
		t.Errorf("expected event type 'LoyaltyPointsRedeemed', got %q", result.EventType)
	}
	if result.Unknown {
		t.Error("expected known event type")
	}
	if result.Fields["points"] != int32(50) {
		t.Errorf("expected points 50, got %v", result.Fields["points"])
	}
	if result.Fields["new_balance"] != int32(50) {
		t.Errorf("expected new_balance 50, got %v", result.Fields["new_balance"])
	}
	if result.Fields["redemption_type"] != "discount" {
		t.Errorf("expected redemption_type 'discount', got %v", result.Fields["redemption_type"])
	}
}

func TestProcessEventBook_UnknownEventType(t *testing.T) {
	logic := NewProjectorLogic()

	// Create an Any with unknown type
	unknownAny := &anypb.Any{
		TypeUrl: "type.googleapis.com/unknown.UnknownEvent",
		Value:   []byte{0x01, 0x02, 0x03},
	}

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{
				Sequence: &angzarr.EventPage_Num{Num: 0},
				Event:    unknownAny,
			},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}

	result := results[0]
	if result.EventType != "UnknownEvent" {
		t.Errorf("expected event type 'UnknownEvent', got %q", result.EventType)
	}
	if !result.Unknown {
		t.Error("expected unknown event type flag to be set")
	}
	if result.Fields["raw_bytes"] != 3 {
		t.Errorf("expected raw_bytes 3, got %v", result.Fields["raw_bytes"])
	}
}

func TestProcessEventBook_MultipleEvents(t *testing.T) {
	logic := NewProjectorLogic()

	created := &examples.CustomerCreated{Name: "Bob", Email: "bob@example.com"}
	createdAny, _ := anypb.New(created)

	added := &examples.LoyaltyPointsAdded{Points: 50, NewBalance: 50}
	addedAny, _ := anypb.New(added)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "customer",
			Root:   &angzarr.UUID{Value: []byte{0x01, 0x02, 0x03, 0x04}},
		},
		Pages: []*angzarr.EventPage{
			{Sequence: &angzarr.EventPage_Num{Num: 0}, Event: createdAny},
			{Sequence: &angzarr.EventPage_Num{Num: 1}, Event: addedAny},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 2 {
		t.Fatalf("expected 2 results, got %d", len(results))
	}

	if results[0].EventType != "CustomerCreated" {
		t.Errorf("expected first event type 'CustomerCreated', got %q", results[0].EventType)
	}
	if results[1].EventType != "LoyaltyPointsAdded" {
		t.Errorf("expected second event type 'LoyaltyPointsAdded', got %q", results[1].EventType)
	}
	if results[0].Sequence != 0 {
		t.Errorf("expected first sequence 0, got %d", results[0].Sequence)
	}
	if results[1].Sequence != 1 {
		t.Errorf("expected second sequence 1, got %d", results[1].Sequence)
	}
}

func TestProcessEventPage_NilPage(t *testing.T) {
	logic := NewProjectorLogic()

	result := logic.ProcessEventPage("customer", "abc123", nil)

	if !result.Unknown {
		t.Error("expected unknown flag for nil page")
	}
}

func TestProcessEventPage_NilEvent(t *testing.T) {
	logic := NewProjectorLogic()

	page := &angzarr.EventPage{
		Sequence: &angzarr.EventPage_Num{Num: 0},
		Event:    nil,
	}

	result := logic.ProcessEventPage("customer", "abc123", page)

	if !result.Unknown {
		t.Error("expected unknown flag for nil event")
	}
}

func TestProcessEventBook_RootIDTruncation(t *testing.T) {
	logic := NewProjectorLogic()

	event := &examples.CustomerCreated{Name: "Test", Email: "test@example.com"}
	eventAny, _ := anypb.New(event)

	// Create a UUID that will result in a long hex string (> 16 chars)
	longUUID := []byte{0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a}

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "customer",
			Root:   &angzarr.UUID{Value: longUUID},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}

	// Root ID should be truncated to 16 characters
	if len(results[0].RootID) != 16 {
		t.Errorf("expected root_id length 16, got %d (%s)", len(results[0].RootID), results[0].RootID)
	}
}

func TestProcessEventBook_DefaultDomain(t *testing.T) {
	logic := NewProjectorLogic()

	event := &examples.CustomerCreated{Name: "Test", Email: "test@example.com"}
	eventAny, _ := anypb.New(event)

	// Event book without cover
	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	results := logic.ProcessEventBook(eventBook)

	if len(results) != 1 {
		t.Fatalf("expected 1 result, got %d", len(results))
	}

	// Should default to "customer"
	if results[0].Domain != "customer" {
		t.Errorf("expected default domain 'customer', got %q", results[0].Domain)
	}
}
