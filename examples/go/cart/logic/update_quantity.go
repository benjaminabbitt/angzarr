package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleUpdateQuantity(state *CartState, productID string, newQuantity int32) (*examples.QuantityUpdated, error) {
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
	if newQuantity <= 0 {
		return nil, NewInvalidArgument(ErrMsgQuantityPositive)
	}

	oldSubtotal := item.Quantity * item.UnitPriceCents
	newItemSubtotal := newQuantity * item.UnitPriceCents
	newSubtotal := state.SubtotalCents - oldSubtotal + newItemSubtotal

	return &examples.QuantityUpdated{
		ProductId:   productID,
		OldQuantity: item.Quantity,
		NewQuantity: newQuantity,
		NewSubtotal: newSubtotal,
		UpdatedAt:   timestamppb.Now(),
	}, nil
}
