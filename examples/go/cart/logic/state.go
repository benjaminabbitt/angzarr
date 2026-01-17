package logic

import (
	"cart/proto/angzarr"
	"cart/proto/examples"
)

type CartItem struct {
	ProductID      string
	Name           string
	Quantity       int32
	UnitPriceCents int32
}

type CartState struct {
	CustomerID    string
	Items         map[string]*CartItem // productID -> item
	SubtotalCents int32
	CouponCode    string
	DiscountCents int32
	Status        string // "active", "checked_out"
}

func (s *CartState) Exists() bool {
	return s.CustomerID != ""
}

func (s *CartState) IsActive() bool {
	return s.Status == "active"
}

func (s *CartState) CalculateSubtotal() int32 {
	var subtotal int32
	for _, item := range s.Items {
		subtotal += item.Quantity * item.UnitPriceCents
	}
	return subtotal
}

func EmptyState() *CartState {
	return &CartState{
		Items: make(map[string]*CartItem),
	}
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

func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
