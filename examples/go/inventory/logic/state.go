package logic

import (
	angzarrpb "angzarr/proto/angzarr"
	"inventory/proto/examples"
)

// RebuildState reconstructs inventory state from an event book.
func RebuildState(eventBook *angzarrpb.EventBook) InventoryState {
	state := InventoryState{
		Reservations: make(map[string]int32),
	}

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.InventoryState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.ProductID = snapState.ProductId
			state.OnHand = snapState.OnHand
			state.Reserved = snapState.Reserved
			state.LowStockThreshold = snapState.LowStockThreshold
			for k, v := range snapState.Reservations {
				state.Reservations[k] = v
			}
		}
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.StockInitialized{}):
			var event examples.StockInitialized
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.ProductID = event.ProductId
				state.OnHand = event.Quantity
				state.LowStockThreshold = event.LowStockThreshold
			}

		case page.Event.MessageIs(&examples.StockReceived{}):
			var event examples.StockReceived
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.OnHand = event.NewOnHand
			}

		case page.Event.MessageIs(&examples.StockReserved{}):
			var event examples.StockReserved
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Reserved += event.Quantity
				state.Reservations[event.OrderId] = event.Quantity
			}

		case page.Event.MessageIs(&examples.ReservationReleased{}):
			var event examples.ReservationReleased
			if err := page.Event.UnmarshalTo(&event); err == nil {
				qty := state.Reservations[event.OrderId]
				state.Reserved -= qty
				delete(state.Reservations, event.OrderId)
			}

		case page.Event.MessageIs(&examples.ReservationCommitted{}):
			var event examples.ReservationCommitted
			if err := page.Event.UnmarshalTo(&event); err == nil {
				qty := state.Reservations[event.OrderId]
				state.OnHand -= qty
				state.Reserved -= qty
				delete(state.Reservations, event.OrderId)
			}
		}
	}

	return state
}
