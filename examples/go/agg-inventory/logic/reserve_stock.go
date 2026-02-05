package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleReserveStock validates and creates StockReserved (and optionally LowStockAlert) events.
func HandleReserveStock(cb *angzarrpb.CommandBook, data []byte, state *InventoryState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.ReserveStock
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotInitialized)
	}
	if cmd.Quantity <= 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgQuantityPositive)
	}
	if cmd.OrderId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgOrderIDRequired)
	}
	if _, exists := state.Reservations[cmd.OrderId]; exists {
		return nil, angzarr.NewFailedPrecondition(ErrMsgReservationExists)
	}
	if state.Available() < cmd.Quantity {
		return nil, angzarr.NewFailedPreconditionf("Insufficient stock: available %d, requested %d", state.Available(), cmd.Quantity)
	}

	newReserved := state.Reserved + cmd.Quantity
	events := []goproto.Message{
		&examples.StockReserved{
			Quantity:     cmd.Quantity,
			OrderId:      cmd.OrderId,
			NewAvailable: state.Available() - cmd.Quantity,
			ReservedAt:   timestamppb.Now(),
			NewReserved:  newReserved,
			NewOnHand:    state.OnHand,
		},
	}

	// Check if we should trigger low stock alert
	newAvailable := state.Available() - cmd.Quantity
	if newAvailable < state.LowStockThreshold && state.Available() >= state.LowStockThreshold {
		events = append(events, &examples.LowStockAlert{
			ProductId: state.ProductID,
			Available: newAvailable,
			Threshold: state.LowStockThreshold,
			AlertedAt: timestamppb.Now(),
		})
	}

	return angzarr.PackEvents(cb.Cover, events, seq)
}
