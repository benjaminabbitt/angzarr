package logic

import (
	"order/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

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
