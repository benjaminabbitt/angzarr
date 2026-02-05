package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

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

	items := make([]*examples.LineItem, 0, len(state.Items))
	for _, item := range state.Items {
		items = append(items, &examples.LineItem{
			ProductId:      item.ProductID,
			Name:           item.Name,
			Quantity:       item.Quantity,
			UnitPriceCents: item.UnitPriceCents,
		})
	}
	return angzarr.PackEvent(cb.Cover, &examples.OrderCancelled{
		Reason:            cmd.Reason,
		CancelledAt:       timestamppb.Now(),
		LoyaltyPointsUsed: state.LoyaltyPointsUsed,
		CustomerRoot:      state.CustomerRoot,
		Items:             items,
		CartRoot:          state.CartRoot,
	}, seq)
}
