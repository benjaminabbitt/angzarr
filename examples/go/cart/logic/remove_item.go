package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleRemoveItem(state *CartState, productID string) (*examples.ItemRemoved, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgCartNotFound)
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
	}

	item, ok := state.Items[productID]
	if !ok {
		return nil, NewFailedPrecondition(ErrMsgItemNotInCart)
	}

	itemSubtotal := item.Quantity * item.UnitPriceCents
	newSubtotal := state.SubtotalCents - itemSubtotal

	return &examples.ItemRemoved{
		ProductId:   productID,
		Quantity:    item.Quantity,
		NewSubtotal: newSubtotal,
		RemovedAt:   timestamppb.Now(),
	}, nil
}
