package logic

import (
	"strings"
	"testing"

	"projector-receipt/proto/angzarr"
	"projector-receipt/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

func TestRebuildState_NilEventBook(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := logic.RebuildState(nil)

	if state.IsComplete() {
		t.Error("expected incomplete state for nil event book")
	}
	if state.CustomerID != "" {
		t.Errorf("expected empty customer ID, got %q", state.CustomerID)
	}
}

func TestRebuildState_EmptyEventBook(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := logic.RebuildState(&angzarr.EventBook{})

	if state.IsComplete() {
		t.Error("expected incomplete state for empty event book")
	}
}

func TestRebuildState_TransactionCreatedOnly(t *testing.T) {
	logic := NewReceiptProjectorLogic()

	event := &examples.TransactionCreated{
		CustomerId: "cust-001",
		Items: []*examples.LineItem{
			{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
		},
		SubtotalCents: 2000,
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.IsComplete() {
		t.Error("expected incomplete state with only TransactionCreated")
	}
	if state.CustomerID != "cust-001" {
		t.Errorf("expected customer ID %q, got %q", "cust-001", state.CustomerID)
	}
	if state.SubtotalCents != 2000 {
		t.Errorf("expected subtotal 2000, got %d", state.SubtotalCents)
	}
	if len(state.Items) != 1 {
		t.Errorf("expected 1 item, got %d", len(state.Items))
	}
}

func TestRebuildState_CompletedTransaction(t *testing.T) {
	logic := NewReceiptProjectorLogic()

	created := &examples.TransactionCreated{
		CustomerId: "cust-001",
		Items: []*examples.LineItem{
			{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
		},
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	completed := &examples.TransactionCompleted{
		FinalTotalCents:     2000,
		PaymentMethod:       "card",
		LoyaltyPointsEarned: 20,
	}
	completedAny, _ := anypb.New(completed)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: completedAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if !state.IsComplete() {
		t.Error("expected complete state")
	}
	if state.FinalTotalCents != 2000 {
		t.Errorf("expected final total 2000, got %d", state.FinalTotalCents)
	}
	if state.PaymentMethod != "card" {
		t.Errorf("expected payment method %q, got %q", "card", state.PaymentMethod)
	}
	if state.LoyaltyPointsEarned != 20 {
		t.Errorf("expected loyalty points 20, got %d", state.LoyaltyPointsEarned)
	}
}

func TestRebuildState_WithDiscount(t *testing.T) {
	logic := NewReceiptProjectorLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-002",
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	discount := &examples.DiscountApplied{
		DiscountType:  "loyalty",
		DiscountCents: 200,
	}
	discountAny, _ := anypb.New(discount)

	completed := &examples.TransactionCompleted{
		FinalTotalCents: 1800,
		PaymentMethod:   "cash",
	}
	completedAny, _ := anypb.New(completed)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: discountAny},
			{Event: completedAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.DiscountCents != 200 {
		t.Errorf("expected discount 200, got %d", state.DiscountCents)
	}
	if state.DiscountType != "loyalty" {
		t.Errorf("expected discount type %q, got %q", "loyalty", state.DiscountType)
	}
	if state.SubtotalCents != 2000 {
		t.Errorf("expected subtotal 2000, got %d", state.SubtotalCents)
	}
	if state.FinalTotalCents != 1800 {
		t.Errorf("expected final total 1800, got %d", state.FinalTotalCents)
	}
}

func TestGenerateReceipt_IncompleteTransaction(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID:    "cust-001",
		SubtotalCents: 2000,
		Completed:     false,
	}

	receipt := logic.GenerateReceipt("tx-001", state)

	if receipt != nil {
		t.Error("expected nil receipt for incomplete transaction")
	}
}

func TestGenerateReceipt_CompletedTransaction(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID: "cust-001",
		Items: []*examples.LineItem{
			{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
		},
		SubtotalCents:       2000,
		FinalTotalCents:     2000,
		PaymentMethod:       "card",
		LoyaltyPointsEarned: 20,
		Completed:           true,
	}

	receipt := logic.GenerateReceipt("tx-001", state)

	if receipt == nil {
		t.Fatal("expected receipt for completed transaction")
	}
	if receipt.TransactionId != "tx-001" {
		t.Errorf("expected transaction ID %q, got %q", "tx-001", receipt.TransactionId)
	}
	if receipt.CustomerId != "cust-001" {
		t.Errorf("expected customer ID %q, got %q", "cust-001", receipt.CustomerId)
	}
	if receipt.FinalTotalCents != 2000 {
		t.Errorf("expected final total 2000, got %d", receipt.FinalTotalCents)
	}
	if receipt.PaymentMethod != "card" {
		t.Errorf("expected payment method %q, got %q", "card", receipt.PaymentMethod)
	}
	if receipt.LoyaltyPointsEarned != 20 {
		t.Errorf("expected loyalty points 20, got %d", receipt.LoyaltyPointsEarned)
	}
	if receipt.FormattedText == "" {
		t.Error("expected non-empty formatted text")
	}
}

func TestGenerateReceipt_WithDiscount(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID:      "cust-002",
		SubtotalCents:   2000,
		DiscountCents:   200,
		DiscountType:    "loyalty",
		FinalTotalCents: 1800,
		PaymentMethod:   "cash",
		Completed:       true,
	}

	receipt := logic.GenerateReceipt("tx-002", state)

	if receipt == nil {
		t.Fatal("expected receipt")
	}
	if receipt.SubtotalCents != 2000 {
		t.Errorf("expected subtotal 2000, got %d", receipt.SubtotalCents)
	}
	if receipt.DiscountCents != 200 {
		t.Errorf("expected discount 200, got %d", receipt.DiscountCents)
	}
	if receipt.FinalTotalCents != 1800 {
		t.Errorf("expected final total 1800, got %d", receipt.FinalTotalCents)
	}
}

func TestFormatReceipt_ContainsHeader(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID:      "cust-001",
		SubtotalCents:   1000,
		FinalTotalCents: 1000,
		PaymentMethod:   "card",
		Completed:       true,
	}

	text := logic.FormatReceipt("tx-001", state)

	if !strings.Contains(text, "RECEIPT") {
		t.Error("expected receipt header")
	}
	if !strings.Contains(text, "Thank you") {
		t.Error("expected thank you message")
	}
}

func TestFormatReceipt_ContainsItems(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID: "cust-001",
		Items: []*examples.LineItem{
			{ProductId: "SKU-001", Name: "Widget", Quantity: 1, UnitPriceCents: 1000},
		},
		SubtotalCents:   1000,
		FinalTotalCents: 1000,
		PaymentMethod:   "card",
		Completed:       true,
	}

	text := logic.FormatReceipt("tx-001", state)

	if !strings.Contains(text, "Widget") {
		t.Error("expected item name in receipt")
	}
}

func TestFormatReceipt_ContainsDiscount(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID:      "cust-001",
		SubtotalCents:   2000,
		DiscountCents:   200,
		DiscountType:    "loyalty",
		FinalTotalCents: 1800,
		PaymentMethod:   "cash",
		Completed:       true,
	}

	text := logic.FormatReceipt("tx-001", state)

	if !strings.Contains(text, "Discount") {
		t.Error("expected discount in receipt")
	}
	if !strings.Contains(text, "loyalty") {
		t.Error("expected discount type in receipt")
	}
}

func TestFormatReceipt_TruncatesLongIDs(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID:      "very-long-customer-id-that-should-be-truncated",
		SubtotalCents:   1000,
		FinalTotalCents: 1000,
		PaymentMethod:   "card",
		Completed:       true,
	}

	longTxID := "very-long-transaction-id-that-should-also-be-truncated"
	text := logic.FormatReceipt(longTxID, state)

	// IDs are truncated to 16 characters
	if strings.Contains(text, longTxID) {
		t.Error("expected transaction ID to be truncated")
	}
	if strings.Contains(text, state.CustomerID) {
		t.Error("expected customer ID to be truncated")
	}
}

func TestFormatReceipt_NoCustomerID(t *testing.T) {
	logic := NewReceiptProjectorLogic()
	state := &TransactionState{
		CustomerID:      "",
		SubtotalCents:   1000,
		FinalTotalCents: 1000,
		PaymentMethod:   "card",
		Completed:       true,
	}

	text := logic.FormatReceipt("tx-001", state)

	if !strings.Contains(text, "Customer: N/A") {
		t.Error("expected N/A for missing customer ID")
	}
}

func TestRebuildState_SkipsNilPages(t *testing.T) {
	logic := NewReceiptProjectorLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: nil},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.CustomerID != "cust-001" {
		t.Errorf("expected customer ID %q, got %q", "cust-001", state.CustomerID)
	}
}
