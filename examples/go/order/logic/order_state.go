package logic

import (
	"order/proto/angzarr"
	"order/proto/examples"
)

type LineItem struct {
	ProductID      string
	Name           string
	Quantity       int32
	UnitPriceCents int32
}

type OrderState struct {
	CustomerID        string
	Items             []LineItem
	SubtotalCents     int32
	DiscountCents     int32
	LoyaltyPointsUsed int32
	PaymentMethod     string
	PaymentReference  string
	Status            string // "pending", "payment_submitted", "completed", "cancelled"
}

func (s *OrderState) Exists() bool {
	return s.CustomerID != ""
}

func (s *OrderState) IsPending() bool {
	return s.Status == "pending"
}

func (s *OrderState) IsPaymentSubmitted() bool {
	return s.Status == "payment_submitted"
}

func (s *OrderState) IsCompleted() bool {
	return s.Status == "completed"
}

func (s *OrderState) IsCancelled() bool {
	return s.Status == "cancelled"
}

func (s *OrderState) TotalAfterDiscount() int32 {
	return s.SubtotalCents - s.DiscountCents
}

func EmptyState() *OrderState {
	return &OrderState{
		Items: make([]LineItem, 0),
	}
}

func RebuildState(eventBook *angzarr.EventBook) *OrderState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.OrderState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.CustomerID = snapState.CustomerId
			state.SubtotalCents = snapState.SubtotalCents
			state.DiscountCents = snapState.DiscountCents
			state.LoyaltyPointsUsed = snapState.LoyaltyPointsUsed
			state.PaymentMethod = snapState.PaymentMethod
			state.PaymentReference = snapState.PaymentReference
			state.Status = snapState.Status
			for _, item := range snapState.Items {
				state.Items = append(state.Items, LineItem{
					ProductID:      item.ProductId,
					Name:           item.Name,
					Quantity:       item.Quantity,
					UnitPriceCents: item.UnitPriceCents,
				})
			}
		}
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.OrderCreated{}):
			var event examples.OrderCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.CustomerID = event.CustomerId
				state.SubtotalCents = event.SubtotalCents
				state.Status = "pending"
				state.Items = make([]LineItem, 0, len(event.Items))
				for _, item := range event.Items {
					state.Items = append(state.Items, LineItem{
						ProductID:      item.ProductId,
						Name:           item.Name,
						Quantity:       item.Quantity,
						UnitPriceCents: item.UnitPriceCents,
					})
				}
			}

		case page.Event.MessageIs(&examples.LoyaltyDiscountApplied{}):
			var event examples.LoyaltyDiscountApplied
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.LoyaltyPointsUsed = event.PointsUsed
				state.DiscountCents = event.DiscountCents
			}

		case page.Event.MessageIs(&examples.PaymentSubmitted{}):
			var event examples.PaymentSubmitted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PaymentMethod = event.PaymentMethod
				state.Status = "payment_submitted"
			}

		case page.Event.MessageIs(&examples.OrderCompleted{}):
			var event examples.OrderCompleted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PaymentReference = event.PaymentReference
				state.Status = "completed"
			}

		case page.Event.MessageIs(&examples.OrderCancelled{}):
			if err := page.Event.UnmarshalTo(&examples.OrderCancelled{}); err == nil {
				state.Status = "cancelled"
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
