package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleCreateCart(state *CartState, customerID string) (*examples.CartCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Cart already exists")
	}
	if customerID == "" {
		return nil, NewInvalidArgument("Customer ID is required")
	}

	return &examples.CartCreated{
		CustomerId: customerID,
		CreatedAt:  timestamppb.Now(),
	}, nil
}

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

func (l *DefaultCartLogic) HandleRemoveItem(state *CartState, productID string) (*examples.ItemRemoved, error) {
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

	itemSubtotal := item.Quantity * item.UnitPriceCents
	newSubtotal := state.SubtotalCents - itemSubtotal

	return &examples.ItemRemoved{
		ProductId:   productID,
		Quantity:    item.Quantity,
		NewSubtotal: newSubtotal,
		RemovedAt:   timestamppb.Now(),
	}, nil
}

func (l *DefaultCartLogic) HandleApplyCoupon(state *CartState, code, couponType string, value int32) (*examples.CouponApplied, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Cart does not exist")
	}
	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cart is already checked out")
	}
	if state.CouponCode != "" {
		return nil, NewFailedPrecondition("Coupon already applied")
	}
	if code == "" {
		return nil, NewInvalidArgument("Coupon code is required")
	}

	var discountCents int32
	switch couponType {
	case "percentage":
		if value < 0 || value > 100 {
			return nil, NewInvalidArgument("Percentage must be 0-100")
		}
		discountCents = (state.SubtotalCents * value) / 100
	case "fixed":
		if value < 0 {
			return nil, NewInvalidArgument("Fixed discount cannot be negative")
		}
		discountCents = value
		if discountCents > state.SubtotalCents {
			discountCents = state.SubtotalCents
		}
	default:
		return nil, NewInvalidArgument("Invalid coupon type")
	}

	return &examples.CouponApplied{
		CouponCode:    code,
		CouponType:    couponType,
		Value:         value,
		DiscountCents: discountCents,
		AppliedAt:     timestamppb.Now(),
	}, nil
}

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
