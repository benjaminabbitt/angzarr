package logic

import (
	"order/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultOrderLogic) HandleCreateOrder(state *OrderState, customerID string, items []*examples.LineItem) (*examples.OrderCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Order already exists")
	}
	if customerID == "" {
		return nil, NewInvalidArgument("Customer ID is required")
	}
	if len(items) == 0 {
		return nil, NewInvalidArgument("Order must have at least one item")
	}

	var subtotal int32
	for _, item := range items {
		if item.Quantity <= 0 {
			return nil, NewInvalidArgument("Item quantity must be positive")
		}
		subtotal += item.Quantity * item.UnitPriceCents
	}

	return &examples.OrderCreated{
		CustomerId:    customerID,
		Items:         items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleApplyLoyaltyDiscount(state *OrderState, points, discountCents int32) (*examples.LoyaltyDiscountApplied, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Order is not in pending state")
	}
	if state.LoyaltyPointsUsed > 0 {
		return nil, NewFailedPrecondition("Loyalty discount already applied")
	}
	if points <= 0 {
		return nil, NewInvalidArgument("Points must be positive")
	}
	if discountCents <= 0 {
		return nil, NewInvalidArgument("Discount must be positive")
	}
	if discountCents > state.SubtotalCents {
		return nil, NewInvalidArgument("Discount cannot exceed subtotal")
	}

	return &examples.LoyaltyDiscountApplied{
		PointsUsed:    points,
		DiscountCents: discountCents,
		AppliedAt:     timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleSubmitPayment(state *OrderState, paymentMethod string, amountCents int32) (*examples.PaymentSubmitted, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if !state.IsPending() {
		return nil, NewFailedPrecondition("Order is not in pending state")
	}
	if paymentMethod == "" {
		return nil, NewInvalidArgument("Payment method is required")
	}
	expectedTotal := state.TotalAfterDiscount()
	if amountCents != expectedTotal {
		return nil, NewInvalidArgument("Payment amount must match order total")
	}

	return &examples.PaymentSubmitted{
		PaymentMethod: paymentMethod,
		AmountCents:   amountCents,
		SubmittedAt:   timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleConfirmPayment(state *OrderState, paymentReference string) (*examples.OrderCompleted, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if !state.IsPaymentSubmitted() {
		return nil, NewFailedPrecondition("Payment not submitted")
	}
	if paymentReference == "" {
		return nil, NewInvalidArgument("Payment reference is required")
	}

	// 1 point per dollar spent
	loyaltyPointsEarned := state.TotalAfterDiscount() / 100

	return &examples.OrderCompleted{
		FinalTotalCents:     state.TotalAfterDiscount(),
		PaymentMethod:       state.PaymentMethod,
		PaymentReference:    paymentReference,
		LoyaltyPointsEarned: loyaltyPointsEarned,
		CompletedAt:         timestamppb.Now(),
	}, nil
}

func (l *DefaultOrderLogic) HandleCancelOrder(state *OrderState, reason string) (*examples.OrderCancelled, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Order does not exist")
	}
	if state.IsCompleted() {
		return nil, NewFailedPrecondition("Cannot cancel completed order")
	}
	if state.IsCancelled() {
		return nil, NewFailedPrecondition("Order already cancelled")
	}
	if reason == "" {
		return nil, NewInvalidArgument("Cancellation reason is required")
	}

	return &examples.OrderCancelled{
		Reason:            reason,
		CancelledAt:       timestamppb.Now(),
		LoyaltyPointsUsed: state.LoyaltyPointsUsed,
	}, nil
}
