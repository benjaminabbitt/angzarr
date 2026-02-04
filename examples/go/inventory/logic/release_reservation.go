package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"inventory/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleReleaseReservation validates and creates a ReservationReleased event.
func HandleReleaseReservation(cb *angzarrpb.CommandBook, data []byte, state *InventoryState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.ReleaseReservation
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

	return angzarr.PackEvent(cb.Cover, &examples.ReservationReleased{
		OrderId:      cmd.OrderId,
		Quantity:     qty,
		NewAvailable: state.Available() + qty,
		ReleasedAt:   timestamppb.Now(),
	}, seq)
}
