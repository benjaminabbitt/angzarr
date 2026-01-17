package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleCheckout(state *CartState) (*examples.CartCheckedOut, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Cart does not exist")
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cart is already checked out")
	}
	if len(state.Items) == 0 {
		return nil, NewFailedPrecondition("Cart is empty")
	}

	return &examples.CartCheckedOut{
		FinalSubtotal: state.SubtotalCents,
		DiscountCents: state.DiscountCents,
		CheckedOutAt:  timestamppb.Now(),
	}, nil
}
