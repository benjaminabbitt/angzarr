package logic

import (
	"cart/proto/angzarr"
	"cart/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type CartLogic interface {
	RebuildState(eventBook *angzarr.EventBook) *CartState
	HandleCreateCart(state *CartState, customerID string) (*examples.CartCreated, error)
	HandleAddItem(state *CartState, productID, name string, quantity, unitPriceCents int32) (*examples.ItemAdded, error)
	HandleUpdateQuantity(state *CartState, productID string, newQuantity int32) (*examples.QuantityUpdated, error)
	HandleRemoveItem(state *CartState, productID string) (*examples.ItemRemoved, error)
	HandleApplyCoupon(state *CartState, code, couponType string, value int32) (*examples.CouponApplied, error)
	HandleClearCart(state *CartState) (*examples.CartCleared, error)
	HandleCheckout(state *CartState) (*examples.CartCheckedOut, error)
}

type DefaultCartLogic struct{}

func NewCartLogic() CartLogic {
	return &DefaultCartLogic{}
}

func (l *DefaultCartLogic) RebuildState(eventBook *angzarr.EventBook) *CartState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.CartState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.CustomerID = snapState.CustomerId
			state.SubtotalCents = snapState.SubtotalCents
			state.CouponCode = snapState.CouponCode
			state.DiscountCents = snapState.DiscountCents
			state.Status = snapState.Status
			for _, item := range snapState.Items {
				state.Items[item.ProductId] = &CartItem{
					ProductID:      item.ProductId,
					Name:           item.Name,
					Quantity:       item.Quantity,
					UnitPriceCents: item.UnitPriceCents,
				}
			}
		}
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.CartCreated{}):
			var event examples.CartCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CustomerID = event.CustomerId
				state.Status = "active"
			}

		case page.Event.MessageIs(&examples.ItemAdded{}):
			var event examples.ItemAdded
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Items[event.ProductId] = &CartItem{
					ProductID:      event.ProductId,
					Name:           event.Name,
					Quantity:       event.Quantity,
					UnitPriceCents: event.UnitPriceCents,
				}
				state.SubtotalCents = event.NewSubtotal
			}

		case page.Event.MessageIs(&examples.QuantityUpdated{}):
			var event examples.QuantityUpdated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				if item, ok := state.Items[event.ProductId]; ok {
					item.Quantity = event.NewQuantity
				}
				state.SubtotalCents = event.NewSubtotal
			}

		case page.Event.MessageIs(&examples.ItemRemoved{}):
			var event examples.ItemRemoved
			if err := page.Event.UnmarshalTo(&event); err == nil {
				delete(state.Items, event.ProductId)
				state.SubtotalCents = event.NewSubtotal
			}

		case page.Event.MessageIs(&examples.CouponApplied{}):
			var event examples.CouponApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CouponCode = event.CouponCode
				state.DiscountCents = event.DiscountCents
			}

		case page.Event.MessageIs(&examples.CartCleared{}):
			var event examples.CartCleared
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Items = make(map[string]*CartItem)
				state.SubtotalCents = 0
				state.CouponCode = ""
				state.DiscountCents = 0
			}

		case page.Event.MessageIs(&examples.CartCheckedOut{}):
			if err := page.Event.UnmarshalTo(&examples.CartCheckedOut{}); err == nil {
				state.Status = "checked_out"
			}
		}
	}

	return state
}

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

func PackEvent(cover *angzarr.Cover, event proto.Message, seq uint32) (*angzarr.EventBook, error) {
	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &angzarr.EventBook{
		Cover: cover,
		Pages: []*angzarr.EventPage{
			{
				Sequence:  &angzarr.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
