package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleApplyCoupon(state *CartState, code, couponType string, value int32) (*examples.CouponApplied, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgCartNotFound)
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
	}
	if state.CouponCode != "" {
		return nil, NewFailedPrecondition(ErrMsgCouponAlreadyApplied)
	}
	if code == "" {
		return nil, NewInvalidArgument(ErrMsgCouponCodeRequired)
	}

	var discountCents int32
	switch couponType {
	case "percentage":
		if value < 0 || value > 100 {
			return nil, NewInvalidArgument(ErrMsgPercentageRange)
		}
		discountCents = (state.SubtotalCents * value) / 100
	case "fixed":
		if value < 0 {
			return nil, NewInvalidArgument(ErrMsgFixedDiscountNeg)
		}
		discountCents = value
		if discountCents > state.SubtotalCents {
			discountCents = state.SubtotalCents
		}
	default:
		return nil, NewInvalidArgument(ErrMsgInvalidCouponType)
	}

	return &examples.CouponApplied{
		CouponCode:    code,
		CouponType:    couponType,
		Value:         value,
		DiscountCents: discountCents,
		AppliedAt:     timestamppb.Now(),
	}, nil
}
