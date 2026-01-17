package logic

import (
	"inventory/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

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
