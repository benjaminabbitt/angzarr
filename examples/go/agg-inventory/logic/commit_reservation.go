package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleCommitReservation validates and creates a ReservationCommitted event.
func HandleCommitReservation(cb *angzarrpb.CommandBook, data []byte, state *InventoryState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.CommitReservation
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotInitialized)
	}
	if cmd.OrderId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgOrderIDRequired)
	}
	qty, exists := state.Reservations[cmd.OrderId]
	if !exists {
		return nil, angzarr.NewFailedPrecondition(ErrMsgReservationNotFound)
	}

	newReserved := state.Reserved - qty
	return angzarr.PackEvent(cb.Cover, &examples.ReservationCommitted{
		OrderId:     cmd.OrderId,
		Quantity:    qty,
		NewOnHand:   state.OnHand - qty,
		CommittedAt: timestamppb.Now(),
		NewReserved: newReserved,
	}, seq)
}
