package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleAddItem(state *CartState, productID, name string, quantity, unitPriceCents int32) (*examples.ItemAdded, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgCartNotFound)
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
	}
	if productID == "" {
		return nil, NewInvalidArgument(ErrMsgProductIDRequired)
	}
	if quantity <= 0 {
		return nil, NewInvalidArgument(ErrMsgQuantityPositive)
	}

	newSubtotal := state.SubtotalCents + (quantity * unitPriceCents)

	return &examples.ItemAdded{
		ProductId:      productID,
		Name:           name,
		Quantity:       quantity,
		UnitPriceCents: unitPriceCents,
		NewSubtotal:    newSubtotal,
		AddedAt:        timestamppb.Now(),
	}, nil
}
