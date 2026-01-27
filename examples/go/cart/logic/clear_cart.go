package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleClearCart(state *CartState) (*examples.CartCleared, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgCartNotFound)
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
	}

	return &examples.CartCleared{
		NewSubtotal: 0,
		ClearedAt:   timestamppb.Now(),
	}, nil
}
