package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleClearCart(state *CartState) (*examples.CartCleared, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Cart does not exist")
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cart is already checked out")
	}

	return &examples.CartCleared{
		NewSubtotal: 0,
		ClearedAt:   timestamppb.Now(),
	}, nil
}
