package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"order/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleApplyLoyaltyDiscount validates and creates a LoyaltyDiscountApplied event.
func HandleApplyLoyaltyDiscount(cb *angzarrpb.CommandBook, data []byte, state *OrderState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.ApplyLoyaltyDiscount
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderNotFound)
	}
	if !state.IsPending() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderNotPending)
	}
	if state.LoyaltyPointsUsed > 0 {
		return nil, angzarr.NewFailedPrecondition(ErrMsgLoyaltyAlready)
	}
	if cmd.Points <= 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgPointsPositive)
	}
	if cmd.DiscountCents <= 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgDiscountPositive)
	}
	if cmd.DiscountCents > state.SubtotalCents {
		return nil, angzarr.NewInvalidArgument(ErrMsgDiscountExceeds)
	}

	return angzarr.PackEvent(cb.Cover, &examples.LoyaltyDiscountApplied{
		PointsUsed:    cmd.Points,
		DiscountCents: cmd.DiscountCents,
		AppliedAt:     timestamppb.Now(),
	}, seq)
}
