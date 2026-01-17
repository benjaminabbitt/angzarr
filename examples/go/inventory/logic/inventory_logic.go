package logic

import (
	"inventory/proto/angzarr"
	"inventory/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type InventoryLogic interface {
	RebuildState(eventBook *angzarr.EventBook) *InventoryState
	HandleInitializeStock(state *InventoryState, productID string, quantity, lowStockThreshold int32) (*examples.StockInitialized, error)
	HandleReceiveStock(state *InventoryState, quantity int32, reference string) (*examples.StockReceived, error)
	HandleReserveStock(state *InventoryState, quantity int32, orderID string) ([]proto.Message, error)
	HandleReleaseReservation(state *InventoryState, orderID string) (*examples.ReservationReleased, error)
	HandleCommitReservation(state *InventoryState, orderID string) (*examples.ReservationCommitted, error)
}

type DefaultInventoryLogic struct{}

func NewInventoryLogic() InventoryLogic {
	return &DefaultInventoryLogic{}
}

func (l *DefaultInventoryLogic) RebuildState(eventBook *angzarr.EventBook) *InventoryState {
	state := EmptyState()

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

func (l *DefaultInventoryLogic) HandleInitializeStock(state *InventoryState, productID string, quantity, lowStockThreshold int32) (*examples.StockInitialized, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Inventory already initialized")
	}
	if productID == "" {
		return nil, NewInvalidArgument("Product ID is required")
	}
	if quantity < 0 {
		return nil, NewInvalidArgument("Quantity cannot be negative")
	}
	if lowStockThreshold < 0 {
		return nil, NewInvalidArgument("Low stock threshold cannot be negative")
	}

	return &examples.StockInitialized{
		ProductId:         productID,
		Quantity:          quantity,
		LowStockThreshold: lowStockThreshold,
		InitializedAt:     timestamppb.Now(),
	}, nil
}

func (l *DefaultInventoryLogic) HandleReceiveStock(state *InventoryState, quantity int32, reference string) (*examples.StockReceived, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Inventory not initialized")
	}
	if quantity <= 0 {
		return nil, NewInvalidArgument("Quantity must be positive")
	}

	return &examples.StockReceived{
		Quantity:   quantity,
		NewOnHand:  state.OnHand + quantity,
		Reference:  reference,
		ReceivedAt: timestamppb.Now(),
	}, nil
}

func (l *DefaultInventoryLogic) HandleReserveStock(state *InventoryState, quantity int32, orderID string) ([]proto.Message, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Inventory not initialized")
	}
	if quantity <= 0 {
		return nil, NewInvalidArgument("Quantity must be positive")
	}
	if orderID == "" {
		return nil, NewInvalidArgument("Order ID is required")
	}
	if _, exists := state.Reservations[orderID]; exists {
		return nil, NewFailedPrecondition("Reservation already exists for this order")
	}
	if state.Available() < quantity {
		return nil, NewFailedPreconditionf("Insufficient stock: available %d, requested %d", state.Available(), quantity)
	}

	events := []proto.Message{
		&examples.StockReserved{
			Quantity:     quantity,
			OrderId:      orderID,
			NewAvailable: state.Available() - quantity,
			ReservedAt:   timestamppb.Now(),
		},
	}

	// Check if we should trigger low stock alert
	newAvailable := state.Available() - quantity
	if newAvailable < state.LowStockThreshold && state.Available() >= state.LowStockThreshold {
		events = append(events, &examples.LowStockAlert{
			ProductId: state.ProductID,
			Available: newAvailable,
			Threshold: state.LowStockThreshold,
			AlertedAt: timestamppb.Now(),
		})
	}

	return events, nil
}

func (l *DefaultInventoryLogic) HandleReleaseReservation(state *InventoryState, orderID string) (*examples.ReservationReleased, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Inventory not initialized")
	}
	if orderID == "" {
		return nil, NewInvalidArgument("Order ID is required")
	}
	qty, exists := state.Reservations[orderID]
	if !exists {
		return nil, NewFailedPrecondition("No reservation found for this order")
	}

	return &examples.ReservationReleased{
		OrderId:      orderID,
		Quantity:     qty,
		NewAvailable: state.Available() + qty,
		ReleasedAt:   timestamppb.Now(),
	}, nil
}

func (l *DefaultInventoryLogic) HandleCommitReservation(state *InventoryState, orderID string) (*examples.ReservationCommitted, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Inventory not initialized")
	}
	if orderID == "" {
		return nil, NewInvalidArgument("Order ID is required")
	}
	qty, exists := state.Reservations[orderID]
	if !exists {
		return nil, NewFailedPrecondition("No reservation found for this order")
	}

	return &examples.ReservationCommitted{
		OrderId:     orderID,
		Quantity:    qty,
		NewOnHand:   state.OnHand - qty,
		CommittedAt: timestamppb.Now(),
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

func PackEvents(cover *angzarr.Cover, events []proto.Message, startSeq uint32) (*angzarr.EventBook, error) {
	pages := make([]*angzarr.EventPage, 0, len(events))
	for i, event := range events {
		eventAny, err := anypb.New(event)
		if err != nil {
			return nil, err
		}
		pages = append(pages, &angzarr.EventPage{
			Sequence:  &angzarr.EventPage_Num{Num: startSeq + uint32(i)},
			Event:     eventAny,
			CreatedAt: timestamppb.Now(),
		})
	}

	return &angzarr.EventBook{
		Cover: cover,
		Pages: pages,
	}, nil
}

func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
