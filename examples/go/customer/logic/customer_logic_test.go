package logic

import (
	"testing"

	"customer/proto/angzarr"
	"customer/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

func TestRebuildState_NilEventBook(t *testing.T) {
	logic := NewCustomerLogic()
	state := logic.RebuildState(nil)

	if state.Exists() {
		t.Error("expected empty state for nil event book")
	}
	if state.Name != "" {
		t.Errorf("expected empty name, got %q", state.Name)
	}
}

func TestRebuildState_EmptyEventBook(t *testing.T) {
	logic := NewCustomerLogic()
	state := logic.RebuildState(&angzarr.EventBook{})

	if state.Exists() {
		t.Error("expected empty state for empty event book")
	}
}

func TestRebuildState_WithCustomerCreated(t *testing.T) {
	logic := NewCustomerLogic()

	event := &examples.CustomerCreated{
		Name:  "John Doe",
		Email: "john@example.com",
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
	if state.Name != "John Doe" {
		t.Errorf("expected name %q, got %q", "John Doe", state.Name)
	}
	if state.Email != "john@example.com" {
		t.Errorf("expected email %q, got %q", "john@example.com", state.Email)
	}
}

func TestRebuildState_WithLoyaltyPointsAdded(t *testing.T) {
	logic := NewCustomerLogic()

	created := &examples.CustomerCreated{Name: "John", Email: "john@example.com"}
	createdAny, _ := anypb.New(created)

	added := &examples.LoyaltyPointsAdded{Points: 100, NewBalance: 100}
	addedAny, _ := anypb.New(added)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: addedAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.LoyaltyPoints != 100 {
		t.Errorf("expected loyalty points 100, got %d", state.LoyaltyPoints)
	}
	if state.LifetimePoints != 100 {
		t.Errorf("expected lifetime points 100, got %d", state.LifetimePoints)
	}
}

func TestRebuildState_WithPointsAddedAndRedeemed(t *testing.T) {
	logic := NewCustomerLogic()

	created := &examples.CustomerCreated{Name: "John", Email: "john@example.com"}
	createdAny, _ := anypb.New(created)

	added := &examples.LoyaltyPointsAdded{Points: 100, NewBalance: 100}
	addedAny, _ := anypb.New(added)

	redeemed := &examples.LoyaltyPointsRedeemed{Points: 30, NewBalance: 70}
	redeemedAny, _ := anypb.New(redeemed)

	eventBook := &angzarr.EventBook{
		Pages: []*angzarr.EventPage{
			{Event: createdAny},
			{Event: addedAny},
			{Event: redeemedAny},
		},
	}

	state := logic.RebuildState(eventBook)

	if state.LoyaltyPoints != 70 {
		t.Errorf("expected loyalty points 70, got %d", state.LoyaltyPoints)
	}
	// Lifetime points should not be reduced by redemptions
	if state.LifetimePoints != 100 {
		t.Errorf("expected lifetime points 100, got %d", state.LifetimePoints)
	}
}

func TestHandleCreateCustomer_Success(t *testing.T) {
	logic := NewCustomerLogic()
	state := EmptyState()

	event, err := logic.HandleCreateCustomer(state, "Jane Doe", "jane@example.com")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.Name != "Jane Doe" {
		t.Errorf("expected name %q, got %q", "Jane Doe", event.Name)
	}
	if event.Email != "jane@example.com" {
		t.Errorf("expected email %q, got %q", "jane@example.com", event.Email)
	}
}

func TestHandleCreateCustomer_AlreadyExists(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "Existing", Email: "existing@test.com"}

	_, err := logic.HandleCreateCustomer(state, "New Name", "new@test.com")

	if err == nil {
		t.Fatal("expected error for existing customer")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleCreateCustomer_EmptyName(t *testing.T) {
	logic := NewCustomerLogic()
	state := EmptyState()

	_, err := logic.HandleCreateCustomer(state, "", "email@test.com")

	if err == nil {
		t.Fatal("expected error for empty name")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleCreateCustomer_EmptyEmail(t *testing.T) {
	logic := NewCustomerLogic()
	state := EmptyState()

	_, err := logic.HandleCreateCustomer(state, "Name", "")

	if err == nil {
		t.Fatal("expected error for empty email")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleAddLoyaltyPoints_Success(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "John", Email: "john@test.com", LoyaltyPoints: 50}

	event, err := logic.HandleAddLoyaltyPoints(state, 25, "purchase")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.Points != 25 {
		t.Errorf("expected points 25, got %d", event.Points)
	}
	if event.NewBalance != 75 {
		t.Errorf("expected new balance 75, got %d", event.NewBalance)
	}
	if event.Reason != "purchase" {
		t.Errorf("expected reason %q, got %q", "purchase", event.Reason)
	}
}

func TestHandleAddLoyaltyPoints_CustomerNotExists(t *testing.T) {
	logic := NewCustomerLogic()
	state := EmptyState()

	_, err := logic.HandleAddLoyaltyPoints(state, 25, "purchase")

	if err == nil {
		t.Fatal("expected error for non-existent customer")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleAddLoyaltyPoints_ZeroPoints(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "John", Email: "john@test.com", LoyaltyPoints: 50}

	_, err := logic.HandleAddLoyaltyPoints(state, 0, "purchase")

	if err == nil {
		t.Fatal("expected error for zero points")
	}
	cmdErr, ok := err.(*CommandError)
	if !ok {
		t.Fatalf("expected CommandError, got %T", err)
	}
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleAddLoyaltyPoints_NegativePoints(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "John", Email: "john@test.com", LoyaltyPoints: 50}

	_, err := logic.HandleAddLoyaltyPoints(state, -10, "purchase")

	if err == nil {
		t.Fatal("expected error for negative points")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}

func TestHandleRedeemLoyaltyPoints_Success(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "John", Email: "john@test.com", LoyaltyPoints: 100}

	event, err := logic.HandleRedeemLoyaltyPoints(state, 50, "discount")

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.Points != 50 {
		t.Errorf("expected points 50, got %d", event.Points)
	}
	if event.NewBalance != 50 {
		t.Errorf("expected new balance 50, got %d", event.NewBalance)
	}
	if event.RedemptionType != "discount" {
		t.Errorf("expected redemption type %q, got %q", "discount", event.RedemptionType)
	}
}

func TestHandleRedeemLoyaltyPoints_CustomerNotExists(t *testing.T) {
	logic := NewCustomerLogic()
	state := EmptyState()

	_, err := logic.HandleRedeemLoyaltyPoints(state, 50, "discount")

	if err == nil {
		t.Fatal("expected error for non-existent customer")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleRedeemLoyaltyPoints_InsufficientPoints(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "John", Email: "john@test.com", LoyaltyPoints: 30}

	_, err := logic.HandleRedeemLoyaltyPoints(state, 50, "discount")

	if err == nil {
		t.Fatal("expected error for insufficient points")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusFailedPrecondition {
		t.Errorf("expected FAILED_PRECONDITION, got %v", cmdErr.Code)
	}
}

func TestHandleRedeemLoyaltyPoints_ZeroPoints(t *testing.T) {
	logic := NewCustomerLogic()
	state := &CustomerState{Name: "John", Email: "john@test.com", LoyaltyPoints: 100}

	_, err := logic.HandleRedeemLoyaltyPoints(state, 0, "discount")

	if err == nil {
		t.Fatal("expected error for zero points")
	}
	cmdErr := err.(*CommandError)
	if cmdErr.Code != StatusInvalidArgument {
		t.Errorf("expected INVALID_ARGUMENT, got %v", cmdErr.Code)
	}
}
