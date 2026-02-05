package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleInitializeStock validates and creates a StockInitialized event.
func HandleInitializeStock(cb *angzarrpb.CommandBook, data []byte, state *InventoryState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.InitializeStock
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgAlreadyInitialized)
	}
	if cmd.ProductId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgProductIDRequired)
	}
	if cmd.Quantity < 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgQuantityNegative)
	}
	if cmd.LowStockThreshold < 0 {
		return nil, angzarr.NewInvalidArgument(ErrMsgThresholdNegative)
	}

	return angzarr.PackEvent(cb.Cover, &examples.StockInitialized{
		ProductId:         cmd.ProductId,
		Quantity:          cmd.Quantity,
		LowStockThreshold: cmd.LowStockThreshold,
		InitializedAt:     timestamppb.Now(),
	}, seq)
}
