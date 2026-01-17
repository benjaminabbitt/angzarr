package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleAddItem(state *CartState, productID, name string, quantity, unitPriceCents int32) (*examples.ItemAdded, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Cart does not exist")
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cart is already checked out")
	}
	if productID == "" {
		return nil, NewInvalidArgument("Product ID is required")
	}
	if quantity <= 0 {
		return nil, NewInvalidArgument("Quantity must be positive")
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
