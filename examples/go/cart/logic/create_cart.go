package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleCreateCart(state *CartState, customerID string) (*examples.CartCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Cart already exists")
	}
	if customerID == "" {
		return nil, NewInvalidArgument("Customer ID is required")
	}

	return &examples.CartCreated{
		CustomerId: customerID,
		CreatedAt:  timestamppb.Now(),
	}, nil
}
