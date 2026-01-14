package logic

import (
	"testing"

	"projector-log-transaction/proto/angzarr"
	"projector-log-transaction/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

func TestProcessEventBook_NilEventBook(t *testing.T) {
	logic := NewLogProjectorLogic()
	entries := logic.ProcessEventBook(nil)

	if entries != nil {
		t.Errorf("expected nil for nil event book, got %v", entries)
	}
}

func TestProcessEventBook_EmptyEventBook(t *testing.T) {
	logic := NewLogProjectorLogic()
	entries := logic.ProcessEventBook(&angzarr.EventBook{})

	if entries != nil {
		t.Errorf("expected nil for empty event book, got %v", entries)
	}
}

func TestProcessEventBook_TransactionCreated(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.TransactionCreated{
		CustomerId:    "cust-12345678901234567890",
		Items:         []*examples.LineItem{{ProductId: "prod-1", Quantity: 2}},
		SubtotalCents: 2000,
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "transaction",
		},
		Pages: []*angzarr.EventPage{
			{
				Sequence: &angzarr.EventPage_Num{Num: 0},
				Event:    eventAny,
			},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	if entry.Domain != "transaction" {
		t.Errorf("expected domain %q, got %q", "transaction", entry.Domain)
	}
	if entry.EventType != "TransactionCreated" {
		t.Errorf("expected event type %q, got %q", "TransactionCreated", entry.EventType)
	}
	if entry.IsUnknown {
		t.Error("expected known event type")
	}

	// Verify customer_id is truncated to 16 chars
	custID, ok := entry.Fields["customer_id"].(string)
	if !ok {
		t.Fatal("expected customer_id to be string")
	}
	if len(custID) > 16 {
		t.Errorf("expected customer_id to be truncated to 16 chars, got %d", len(custID))
	}

	itemCount, ok := entry.Fields["item_count"].(int)
	if !ok {
		t.Fatal("expected item_count to be int")
	}
	if itemCount != 1 {
		t.Errorf("expected item_count 1, got %d", itemCount)
	}

	subtotal, ok := entry.Fields["subtotal_cents"].(int32)
	if !ok {
		t.Fatal("expected subtotal_cents to be int32")
	}
	if subtotal != 2000 {
		t.Errorf("expected subtotal_cents 2000, got %d", subtotal)
	}
}

func TestProcessEventBook_TransactionCompleted(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.TransactionCompleted{
		FinalTotalCents:     1800,
		PaymentMethod:       "card",
		LoyaltyPointsEarned: 18,
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

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	if entry.EventType != "TransactionCompleted" {
		t.Errorf("expected event type %q, got %q", "TransactionCompleted", entry.EventType)
	}
	if entry.Sequence != 1 {
		t.Errorf("expected sequence 1, got %d", entry.Sequence)
	}
	if entry.IsUnknown {
		t.Error("expected known event type")
	}

	finalTotal, ok := entry.Fields["final_total_cents"].(int32)
	if !ok {
		t.Fatal("expected final_total_cents to be int32")
	}
	if finalTotal != 1800 {
		t.Errorf("expected final_total_cents 1800, got %d", finalTotal)
	}

	paymentMethod, ok := entry.Fields["payment_method"].(string)
	if !ok {
		t.Fatal("expected payment_method to be string")
	}
	if paymentMethod != "card" {
		t.Errorf("expected payment_method %q, got %q", "card", paymentMethod)
	}
}

func TestProcessEventBook_DiscountApplied(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.DiscountApplied{
		DiscountType:  "percentage",
		Value:         10,
		DiscountCents: 200,
		CouponCode:    "SAVE10",
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	if entry.EventType != "DiscountApplied" {
		t.Errorf("expected event type %q, got %q", "DiscountApplied", entry.EventType)
	}
	if entry.IsUnknown {
		t.Error("expected known event type")
	}

	discountType, ok := entry.Fields["discount_type"].(string)
	if !ok {
		t.Fatal("expected discount_type to be string")
	}
	if discountType != "percentage" {
		t.Errorf("expected discount_type %q, got %q", "percentage", discountType)
	}

	couponCode, ok := entry.Fields["coupon_code"].(string)
	if !ok {
		t.Fatal("expected coupon_code to be string")
	}
	if couponCode != "SAVE10" {
		t.Errorf("expected coupon_code %q, got %q", "SAVE10", couponCode)
	}
}

func TestProcessEventBook_TransactionCancelled(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.TransactionCancelled{
		Reason: "customer request",
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	if entry.EventType != "TransactionCancelled" {
		t.Errorf("expected event type %q, got %q", "TransactionCancelled", entry.EventType)
	}
	if entry.IsUnknown {
		t.Error("expected known event type")
	}

	reason, ok := entry.Fields["reason"].(string)
	if !ok {
		t.Fatal("expected reason to be string")
	}
	if reason != "customer request" {
		t.Errorf("expected reason %q, got %q", "customer request", reason)
	}
}

func TestProcessEventBook_UnknownEventType(t *testing.T) {
	logic := NewLogProjectorLogic()

	// Use CustomerCreated as an "unknown" event for transaction projector
	event := &examples.CustomerCreated{
		Name:  "Alice",
		Email: "alice@example.com",
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		t.Fatalf("failed to create Any: %v", err)
	}

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	if entry.EventType != "CustomerCreated" {
		t.Errorf("expected event type %q, got %q", "CustomerCreated", entry.EventType)
	}
	if !entry.IsUnknown {
		t.Error("expected unknown event type flag to be set")
	}

	rawBytes, ok := entry.Fields["raw_bytes"].(int)
	if !ok {
		t.Fatal("expected raw_bytes to be int")
	}
	if rawBytes == 0 {
		t.Error("expected raw_bytes to be non-zero")
	}
}

func TestProcessEventBook_MultipleEvents(t *testing.T) {
	logic := NewLogProjectorLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 1000,
	}
	createdAny, _ := anypb.New(created)

	completed := &examples.TransactionCompleted{
		FinalTotalCents: 1000,
		PaymentMethod:   "cash",
	}
	completedAny, _ := anypb.New(completed)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Sequence: &angzarr.EventPage_Num{Num: 0}, Event: createdAny},
			{Sequence: &angzarr.EventPage_Num{Num: 1}, Event: completedAny},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(entries))
	}

	if entries[0].EventType != "TransactionCreated" {
		t.Errorf("expected first event type %q, got %q", "TransactionCreated", entries[0].EventType)
	}
	if entries[0].Sequence != 0 {
		t.Errorf("expected first sequence 0, got %d", entries[0].Sequence)
	}

	if entries[1].EventType != "TransactionCompleted" {
		t.Errorf("expected second event type %q, got %q", "TransactionCompleted", entries[1].EventType)
	}
	if entries[1].Sequence != 1 {
		t.Errorf("expected second sequence 1, got %d", entries[1].Sequence)
	}
}

func TestProcessEventBook_WithCover(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.TransactionCreated{CustomerId: "cust-001"}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Cover: &angzarr.Cover{
			Domain: "custom-domain",
			Root:   &angzarr.UUID{Value: []byte{0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10}},
		},
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	entry := entries[0]
	if entry.Domain != "custom-domain" {
		t.Errorf("expected domain %q, got %q", "custom-domain", entry.Domain)
	}
	if entry.RootID != "0102030405060708" {
		t.Errorf("expected root_id %q, got %q", "0102030405060708", entry.RootID)
	}
}

func TestProcessEventBook_SkipsNilEvents(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.TransactionCreated{CustomerId: "cust-001"}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: nil},
			{Event: eventAny},
			{Event: nil},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry (skipping nil events), got %d", len(entries))
	}

	if entries[0].EventType != "TransactionCreated" {
		t.Errorf("expected event type %q, got %q", "TransactionCreated", entries[0].EventType)
	}
}

func TestProcessEventBook_DefaultDomain(t *testing.T) {
	logic := NewLogProjectorLogic()

	event := &examples.TransactionCreated{CustomerId: "cust-001"}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	entries := logic.ProcessEventBook(eventBook)

	if len(entries) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(entries))
	}

	if entries[0].Domain != "transaction" {
		t.Errorf("expected default domain %q, got %q", "transaction", entries[0].Domain)
	}
}
