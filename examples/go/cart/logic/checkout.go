package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleCheckout(state *CartState) (*examples.CartCheckedOut, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgCartNotFound)
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
	}
	if len(state.Items) == 0 {
		return nil, NewFailedPrecondition(ErrMsgCartEmpty)
	}

	return &examples.CartCheckedOut{
		FinalSubtotal: state.SubtotalCents,
		DiscountCents: state.DiscountCents,
		CheckedOutAt:  timestamppb.Now(),
	}, nil
}
