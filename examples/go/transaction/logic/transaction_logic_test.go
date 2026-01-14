package logic

import (
	"testing"

	"transaction/proto/angzarr"
	"transaction/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

func TestRebuildState_NilEventBook(t *testing.T) {
	logic := NewTransactionLogic()
	state := logic.RebuildState(nil)

	if state.Exists() {
		t.Error("expected empty state for nil event book")
	}
	if state.Status != "new" {
		t.Errorf("expected status 'new', got %q", state.Status)
	}
}

func TestRebuildState_EmptyEventBook(t *testing.T) {
	logic := NewTransactionLogic()
	state := logic.RebuildState(&angzarr.EventBook{})

	if state.Exists() {
		t.Error("expected empty state for empty event book")
	}
	if state.Status != "new" {
		t.Errorf("expected status 'new', got %q", state.Status)
	}
}

func TestRebuildState_WithTransactionCreated(t *testing.T) {
	logic := NewTransactionLogic()

	event := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
		Items: []*examples.LineItem{
			{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
		},
	}
	eventAny, _ := anypb.New(event)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: eventAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if !state.Exists() {
		t.Error("expected state to exist")
	}
	if state.CustomerID != "cust-001" {
		t.Errorf("expected customer_id %q, got %q", "cust-001", state.CustomerID)
	}
	if state.SubtotalCents != 2000 {
		t.Errorf("expected subtotal_cents %d, got %d", 2000, state.SubtotalCents)
	}
	if state.Status != "pending" {
		t.Errorf("expected status 'pending', got %q", state.Status)
	}
}

func TestRebuildState_WithDiscountApplied(t *testing.T) {
	logic := NewTransactionLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	discount := &examples.DiscountApplied{
		DiscountType:  "percentage",
		DiscountCents: 200,
	}
	discountAny, _ := anypb.New(discount)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: discountAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.DiscountCents != 200 {
		t.Errorf("expected discount_cents %d, got %d", 200, state.DiscountCents)
	}
	if state.DiscountType != "percentage" {
		t.Errorf("expected discount_type %q, got %q", "percentage", state.DiscountType)
	}
}

func TestRebuildState_WithTransactionCompleted(t *testing.T) {
	logic := NewTransactionLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	completed := &examples.TransactionCompleted{
		FinalTotalCents: 2000,
		PaymentMethod:   "card",
	}
	completedAny, _ := anypb.New(completed)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: completedAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.Status != "completed" {
		t.Errorf("expected status 'completed', got %q", state.Status)
	}
}

func TestRebuildState_WithTransactionCancelled(t *testing.T) {
	logic := NewTransactionLogic()

	created := &examples.TransactionCreated{
		CustomerId:    "cust-001",
		SubtotalCents: 2000,
	}
	createdAny, _ := anypb.New(created)

	cancelled := &examples.TransactionCancelled{
		Reason: "customer request",
	}
	cancelledAny, _ := anypb.New(cancelled)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: cancelledAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.Status != "cancelled" {
		t.Errorf("expected status 'cancelled', got %q", state.Status)
	}
}

func TestHandleCreateTransaction_Success(t *testing.T) {
	logic := NewTransactionLogic()
	state := EmptyState()

	items := []*examples.LineItem{
		{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
	}

	event, err := logic.HandleCreateTransaction(state, "cust-001", items)

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.CustomerId != "cust-001" {
		t.Errorf("expected customer_id %q, got %q", "cust-001", event.CustomerId)
	}
	if event.SubtotalCents != 2000 {
		t.Errorf("expected subtotal_cents %d, got %d", 2000, event.SubtotalCents)
	}
}

func TestHandleCreateTransaction_MultipleItems(t *testing.T) {
	logic := NewTransactionLogic()
	state := EmptyState()

	items := []*examples.LineItem{
		{ProductId: "SKU-001", Name: "Widget", Quantity: 2, UnitPriceCents: 1000},
		{ProductId: "SKU-002", Name: "Gadget", Quantity: 1, UnitPriceCents: 2500},
	}

	event, err := logic.HandleCreateTransaction(state, "cust-001", items)

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.SubtotalCents != 4500 {
		t.Errorf("expected subtotal_cents %d, got %d", 4500, event.SubtotalCents)
	}
}

func TestHandleCreateTransaction_AlreadyExists(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{Status: "pending"}

	items := []*examples.LineItem{
		{ProductId: "SKU-001", Name: "Widget", Quantity: 1, UnitPriceCents: 1000},
	}

	_, err := logic.HandleCreateTransaction(state, "cust-001", items)

	if err == nil {
		t.Fatal("expected error for existing transaction")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleCreateTransaction_EmptyCustomerID(t *testing.T) {
	logic := NewTransactionLogic()
	state := EmptyState()

	items := []*examples.LineItem{
		{ProductId: "SKU-001", Name: "Widget", Quantity: 1, UnitPriceCents: 1000},
	}

	_, err := logic.HandleCreateTransaction(state, "", items)

	if err == nil {
		t.Fatal("expected error for empty customer ID")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleCreateTransaction_NoItems(t *testing.T) {
	logic := NewTransactionLogic()
	state := EmptyState()

	_, err := logic.HandleCreateTransaction(state, "cust-001", nil)

	if err == nil {
		t.Fatal("expected error for no items")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleApplyDiscount_Percentage(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
	}

	event, err := logic.HandleApplyDiscount(state, "percentage", 10, "")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.DiscountCents != 200 {
		t.Errorf("expected discount_cents %d, got %d", 200, event.DiscountCents)
	}
	if event.DiscountType != "percentage" {
		t.Errorf("expected discount_type %q, got %q", "percentage", event.DiscountType)
	}
}

func TestHandleApplyDiscount_Fixed(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
	}

	event, err := logic.HandleApplyDiscount(state, "fixed", 500, "")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.DiscountCents != 500 {
		t.Errorf("expected discount_cents %d, got %d", 500, event.DiscountCents)
	}
}

func TestHandleApplyDiscount_FixedCappedAtSubtotal(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 300,
	}

	event, err := logic.HandleApplyDiscount(state, "fixed", 500, "")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.DiscountCents != 300 {
		t.Errorf("expected discount_cents %d (capped at subtotal), got %d", 300, event.DiscountCents)
	}
}

func TestHandleApplyDiscount_Coupon(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
	}

	event, err := logic.HandleApplyDiscount(state, "coupon", 0, "SAVE5")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.DiscountCents != 500 {
		t.Errorf("expected discount_cents %d, got %d", 500, event.DiscountCents)
	}
	if event.CouponCode != "SAVE5" {
		t.Errorf("expected coupon_code %q, got %q", "SAVE5", event.CouponCode)
	}
}

func TestHandleApplyDiscount_NotPending(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "completed",
		SubtotalCents: 2000,
	}

	_, err := logic.HandleApplyDiscount(state, "percentage", 10, "")

	if err == nil {
		t.Fatal("expected error for non-pending transaction")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleApplyDiscount_InvalidPercentage(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
	}

	_, err := logic.HandleApplyDiscount(state, "percentage", 150, "")

	if err == nil {
		t.Fatal("expected error for invalid percentage")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleApplyDiscount_UnknownType(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
	}

	_, err := logic.HandleApplyDiscount(state, "unknown", 10, "")

	if err == nil {
		t.Fatal("expected error for unknown discount type")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleCompleteTransaction_Success(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
		DiscountCents: 0,
	}

	event, err := logic.HandleCompleteTransaction(state, "card")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.FinalTotalCents != 2000 {
		t.Errorf("expected final_total_cents %d, got %d", 2000, event.FinalTotalCents)
	}
	if event.PaymentMethod != "card" {
		t.Errorf("expected payment_method %q, got %q", "card", event.PaymentMethod)
	}
	if event.LoyaltyPointsEarned != 20 {
		t.Errorf("expected loyalty_points_earned %d, got %d", 20, event.LoyaltyPointsEarned)
	}
}

func TestHandleCompleteTransaction_WithDiscount(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
		DiscountCents: 200,
	}

	event, err := logic.HandleCompleteTransaction(state, "cash")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.FinalTotalCents != 1800 {
		t.Errorf("expected final_total_cents %d, got %d", 1800, event.FinalTotalCents)
	}
	if event.LoyaltyPointsEarned != 18 {
		t.Errorf("expected loyalty_points_earned %d, got %d", 18, event.LoyaltyPointsEarned)
	}
}

func TestHandleCompleteTransaction_NotPending(t *testing.T) {
	logic := NewTransactionLogic()
	state := EmptyState()

	_, err := logic.HandleCompleteTransaction(state, "card")

	if err == nil {
		t.Fatal("expected error for non-pending transaction")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleCancelTransaction_Success(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status:        "pending",
		SubtotalCents: 2000,
	}

	event, err := logic.HandleCancelTransaction(state, "customer request")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.Reason != "customer request" {
		t.Errorf("expected reason %q, got %q", "customer request", event.Reason)
	}
}

func TestHandleCancelTransaction_NotPending(t *testing.T) {
	logic := NewTransactionLogic()
	state := &TransactionState{
		Status: "completed",
	}

	_, err := logic.HandleCancelTransaction(state, "too late")

	if err == nil {
		t.Fatal("expected error for non-pending transaction")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestNextSequence_NilEventBook(t *testing.T) {
	seq := NextSequence(nil)
	if seq != 0 {
		t.Errorf("expected sequence 0, got %d", seq)
	}
}

func TestNextSequence_EmptyEventBook(t *testing.T) {
	seq := NextSequence(&angzarr.EventBook{})
	if seq != 0 {
		t.Errorf("expected sequence 0, got %d", seq)
	}
}

func TestNextSequence_WithEvents(t *testing.T) {
	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{},
			{},
			{},
		},
	}
	seq := NextSequence(eventBook)
	if seq != 3 {
		t.Errorf("expected sequence 3, got %d", seq)
	}
}
