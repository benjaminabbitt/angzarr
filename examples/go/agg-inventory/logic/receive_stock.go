package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"inventory/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleReceiveStock validates and creates a StockReceived event.
func HandleReceiveStock(cb *angzarrpb.CommandBook, data []byte, state *InventoryState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.ReceiveStock
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotInitialized)
	}
	if cmd.Quantity <= 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgQuantityPositive)
	}

	return angzarr.PackEvent(cb.Cover, &examples.StockReceived{
		Quantity:   cmd.Quantity,
		NewOnHand:  state.OnHand + cmd.Quantity,
		Reference:  cmd.Reference,
		ReceivedAt: timestamppb.Now(),
	}, seq)
}
