package logic

import (
	"order/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

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
