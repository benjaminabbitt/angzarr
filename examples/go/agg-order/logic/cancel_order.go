package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"order/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleCancelOrder validates and creates an OrderCancelled event.
func HandleCancelOrder(cb *angzarrpb.CommandBook, data []byte, state *OrderState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.CancelOrder
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderNotFound)
	}
	if state.IsCompleted() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgCannotCancelDone)
	}
	if state.IsCancelled() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgAlreadyCancelled)
	}
	if cmd.Reason == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgReasonRequired)
	}

	return angzarr.PackEvent(cb.Cover, &examples.OrderCancelled{
		Reason:            cmd.Reason,
		CancelledAt:       timestamppb.Now(),
		LoyaltyPointsUsed: state.LoyaltyPointsUsed,
	}, seq)
}
