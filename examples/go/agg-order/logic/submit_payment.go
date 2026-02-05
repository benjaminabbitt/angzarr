package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleSubmitPayment validates and creates a PaymentSubmitted event.
func HandleSubmitPayment(cb *angzarrpb.CommandBook, data []byte, state *OrderState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.SubmitPayment
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderNotFound)
	}
	if !state.IsPending() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderNotPending)
	}
	if cmd.PaymentMethod == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgPaymentMethodReq)
	}
	expectedTotal := state.TotalAfterDiscount()
	if cmd.AmountCents != expectedTotal {
		return nil, angzarr.NewInvalidArgument(ErrMsgPaymentAmountMatch)
	}

	return angzarr.PackEvent(cb.Cover, &examples.PaymentSubmitted{
		PaymentMethod: cmd.PaymentMethod,
		AmountCents:   cmd.AmountCents,
		SubmittedAt:   timestamppb.Now(),
	}, seq)
}
