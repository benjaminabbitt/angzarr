package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"order/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleCreateOrder validates and creates an OrderCreated event.
func HandleCreateOrder(cb *angzarrpb.CommandBook, data []byte, state *OrderState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.CreateOrder
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgOrderExists)
	}
	if cmd.CustomerId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgCustomerIDRequired)
	}
	if len(cmd.Items) == 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgItemsRequired)
	}

	var subtotal int32
	for _, item := range cmd.Items {
		if item.Quantity <= 0 {
			return nil, angzarr.NewInvalidArgument(ErrMsgItemQuantityPos)
		}
		subtotal += item.Quantity * item.UnitPriceCents
	}

	return angzarr.PackEvent(cb.Cover, &examples.OrderCreated{
		CustomerId:    cmd.CustomerId,
		Items:         cmd.Items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
	}, seq)
}
