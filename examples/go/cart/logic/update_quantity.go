package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleUpdateQuantity(state *CartState, productID string, newQuantity int32) (*examples.QuantityUpdated, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Cart does not exist")
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cart is already checked out")
	}

	item, ok := state.Items[productID]
	if !ok {
		return nil, NewFailedPrecondition("Item not in cart")
	}
	if newQuantity <= 0 {
		return nil, NewInvalidArgument("Quantity must be positive")
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
