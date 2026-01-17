package logic

import (
	"order/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

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
