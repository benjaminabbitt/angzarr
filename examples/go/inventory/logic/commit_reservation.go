package logic

import (
	"inventory/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

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
