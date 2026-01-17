package logic

import (
	"cart/proto/angzarr"
	"cart/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type CartLogic interface {
	RebuildState(eventBook *angzarr.EventBook) *CartState
	HandleCreateCart(state *CartState, customerID string) (*examples.CartCreated, error)
	HandleAddItem(state *CartState, productID, name string, quantity, unitPriceCents int32) (*examples.ItemAdded, error)
	HandleUpdateQuantity(state *CartState, productID string, newQuantity int32) (*examples.QuantityUpdated, error)
	HandleRemoveItem(state *CartState, productID string) (*examples.ItemRemoved, error)
	HandleApplyCoupon(state *CartState, code, couponType string, value int32) (*examples.CouponApplied, error)
	HandleClearCart(state *CartState) (*examples.CartCleared, error)
	HandleCheckout(state *CartState) (*examples.CartCheckedOut, error)
}

type DefaultCartLogic struct{}

func NewCartLogic() CartLogic {
	return &DefaultCartLogic{}
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
