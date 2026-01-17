package logic

import (
	"inventory/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

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
