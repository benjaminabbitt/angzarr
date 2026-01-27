package logic

import (
	"cart/proto/examples"

	"google.golang.org/protobuf/types/known/timestamppb"
)

func (l *DefaultCartLogic) HandleCreateCart(state *CartState, customerID string) (*examples.CartCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgCartExists)
	}
	if customerID == "" {
		return nil, NewInvalidArgument(ErrMsgCustomerIDRequired)
	}

	return &examples.CartCreated{
		CustomerId: customerID,
		CreatedAt:  timestamppb.Now(),
	}, nil
}
