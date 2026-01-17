package logic

import (
	"order/proto/angzarr"
	"order/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type OrderLogic interface {
	HandleCreateOrder(state *OrderState, customerID string, items []*examples.LineItem) (*examples.OrderCreated, error)
	HandleApplyLoyaltyDiscount(state *OrderState, points, discountCents int32) (*examples.LoyaltyDiscountApplied, error)
	HandleSubmitPayment(state *OrderState, paymentMethod string, amountCents int32) (*examples.PaymentSubmitted, error)
	HandleConfirmPayment(state *OrderState, paymentReference string) (*examples.OrderCompleted, error)
	HandleCancelOrder(state *OrderState, reason string) (*examples.OrderCancelled, error)
}

type DefaultOrderLogic struct{}

func NewOrderLogic() OrderLogic {
	return &DefaultOrderLogic{}
}

func PackEvent(cover *angzarr.Cover, event proto.Message, seq uint32) (*angzarr.EventBook, error) {
	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &angzarr.EventBook{
		Cover: cover,
		Pages: []*angzarr.EventPage{
			{
				Sequence:  &angzarr.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}
