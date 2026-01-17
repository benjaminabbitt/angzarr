package logic

import (
	"order/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

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
