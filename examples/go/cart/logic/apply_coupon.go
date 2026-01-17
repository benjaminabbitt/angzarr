package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleApplyCoupon(state *CartState, code, couponType string, value int32) (*examples.CouponApplied, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Cart does not exist")
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cart is already checked out")
	}
	if state.CouponCode != "" {
		return nil, NewFailedPrecondition("Coupon already applied")
	}
	if code == "" {
		return nil, NewInvalidArgument("Coupon code is required")
	}

	var discountCents int32
	switch couponType {
	case "percentage":
		if value < 0 || value > 100 {
			return nil, NewInvalidArgument("Percentage must be 0-100")
		}
		discountCents = (state.SubtotalCents * value) / 100
	case "fixed":
		if value < 0 {
			return nil, NewInvalidArgument("Fixed discount cannot be negative")
		}
		discountCents = value
		if discountCents > state.SubtotalCents {
			discountCents = state.SubtotalCents
		}
	default:
		return nil, NewInvalidArgument("Invalid coupon type")
	}

	return &examples.CouponApplied{
		CouponCode:    code,
		CouponType:    couponType,
		Value:         value,
		DiscountCents: discountCents,
		AppliedAt:     timestamppb.Now(),
	}, nil
}
