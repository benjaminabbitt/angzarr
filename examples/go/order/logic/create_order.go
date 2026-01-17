package logic

import (
	"order/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultOrderLogic) HandleCreateOrder(state *OrderState, customerID string, items []*examples.LineItem) (*examples.OrderCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Order already exists")
	}
	if customerID == "" {
		return nil, NewInvalidArgument("Customer ID is required")
	}
	if len(items) == 0 {
		return nil, NewInvalidArgument("Order must have at least one item")
	}

	var subtotal int32
	for _, item := range items {
		if item.Quantity <= 0 {
			return nil, NewInvalidArgument("Item quantity must be positive")
		}
		subtotal += item.Quantity * item.UnitPriceCents
	}

	return &examples.OrderCreated{
		CustomerId:    customerID,
		Items:         items,
		SubtotalCents: subtotal,
		CreatedAt:     timestamppb.Now(),
	}, nil
}
