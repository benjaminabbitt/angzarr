package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"order/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleConfirmPayment validates and creates an OrderCompleted event.
func HandleConfirmPayment(cb *angzarrpb.CommandBook, data []byte, state *OrderState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.ConfirmPayment
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderNotFound)
	}
	if !state.IsPaymentSubmitted() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgPaymentNotSubmitted)
	}
	if cmd.PaymentReference == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgPaymentRefRequired)
	}

	// 1 point per dollar spent
	loyaltyPointsEarned := state.TotalAfterDiscount() / 100

	return angzarr.PackEvent(cb.Cover, &examples.OrderCompleted{
		FinalTotalCents:     state.TotalAfterDiscount(),
		PaymentMethod:       state.PaymentMethod,
		PaymentReference:    cmd.PaymentReference,
		LoyaltyPointsEarned: loyaltyPointsEarned,
		CompletedAt:         timestamppb.Now(),
	}, seq)
}
