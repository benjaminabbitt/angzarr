package logic

import (
	"inventory/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

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
