package logic

import (
	"inventory/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultInventoryLogic) HandleReceiveStock(state *InventoryState, quantity int32, reference string) (*examples.StockReceived, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Inventory not initialized")
	}
	if quantity <= 0 {
		return nil, NewInvalidArgument("Quantity must be positive")
	}

	return &examples.StockReceived{
		Quantity:   quantity,
		NewOnHand:  state.OnHand + quantity,
		Reference:  reference,
		ReceivedAt: timestamppb.Now(),
	}, nil
}
