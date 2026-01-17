package logic

import (
	"inventory/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultInventoryLogic) HandleInitializeStock(state *InventoryState, productID string, quantity, lowStockThreshold int32) (*examples.StockInitialized, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Inventory already initialized")
	}
	if productID == "" {
		return nil, NewInvalidArgument("Product ID is required")
	}
	if quantity < 0 {
		return nil, NewInvalidArgument("Quantity cannot be negative")
	}
	if lowStockThreshold < 0 {
		return nil, NewInvalidArgument("Low stock threshold cannot be negative")
	}

	return &examples.StockInitialized{
		ProductId:         productID,
		Quantity:          quantity,
		LowStockThreshold: lowStockThreshold,
		InitializedAt:     timestamppb.Now(),
	}, nil
}
