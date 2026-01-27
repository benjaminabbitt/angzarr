package logic

import (
	"fulfillment/proto/angzarr"
	"fulfillment/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type FulfillmentLogic interface {
	RebuildState(eventBook *angzarr.EventBook) *FulfillmentState
	HandleCreateShipment(state *FulfillmentState, orderID string) (*examples.ShipmentCreated, error)
	HandleMarkPicked(state *FulfillmentState, pickerID string) (*examples.ItemsPicked, error)
	HandleMarkPacked(state *FulfillmentState, packerID string) (*examples.ItemsPacked, error)
	HandleShip(state *FulfillmentState, carrier, trackingNumber string) (*examples.Shipped, error)
	HandleRecordDelivery(state *FulfillmentState, signature string) (*examples.Delivered, error)
}

type DefaultFulfillmentLogic struct{}

func NewFulfillmentLogic() FulfillmentLogic {
	return &DefaultFulfillmentLogic{}
}

func (l *DefaultFulfillmentLogic) RebuildState(eventBook *angzarr.EventBook) *FulfillmentState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.FulfillmentState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.OrderID = snapState.OrderId
			state.Status = snapState.Status
			state.TrackingNumber = snapState.TrackingNumber
			state.Carrier = snapState.Carrier
			state.PickerID = snapState.PickerId
			state.PackerID = snapState.PackerId
			state.Signature = snapState.Signature
		}
	}

	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.ShipmentCreated{}):
			var event examples.ShipmentCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.OrderID = event.OrderId
				state.Status = event.Status
			}

		case page.Event.MessageIs(&examples.ItemsPicked{}):
			var event examples.ItemsPicked
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PickerID = event.PickerId
				state.Status = "picking"
			}

		case page.Event.MessageIs(&examples.ItemsPacked{}):
			var event examples.ItemsPacked
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PackerID = event.PackerId
				state.Status = "packing"
			}

		case page.Event.MessageIs(&examples.Shipped{}):
			var event examples.Shipped
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Carrier = event.Carrier
				state.TrackingNumber = event.TrackingNumber
				state.Status = "shipped"
			}

		case page.Event.MessageIs(&examples.Delivered{}):
			var event examples.Delivered
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Signature = event.Signature
				state.Status = "delivered"
			}
		}
	}

	return state
}

func (l *DefaultFulfillmentLogic) HandleCreateShipment(state *FulfillmentState, orderID string) (*examples.ShipmentCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgShipmentExists)
	}
	if orderID == "" {
		return nil, NewInvalidArgument(ErrMsgOrderIDRequired)
	}

	return &examples.ShipmentCreated{
		OrderId:   orderID,
		Status:    "pending",
		CreatedAt: timestamppb.Now(),
	}, nil
}

func (l *DefaultFulfillmentLogic) HandleMarkPicked(state *FulfillmentState, pickerID string) (*examples.ItemsPicked, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsPending() {
		return nil, NewFailedPrecondition(ErrMsgNotPending)
	}
	if pickerID == "" {
		return nil, NewInvalidArgument(ErrMsgPickerIDRequired)
	}

	return &examples.ItemsPicked{
		PickerId: pickerID,
		PickedAt: timestamppb.Now(),
	}, nil
}

func (l *DefaultFulfillmentLogic) HandleMarkPacked(state *FulfillmentState, packerID string) (*examples.ItemsPacked, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsPicking() {
		return nil, NewFailedPrecondition(ErrMsgNotPicked)
	}
	if packerID == "" {
		return nil, NewInvalidArgument(ErrMsgPackerIDRequired)
	}

	return &examples.ItemsPacked{
		PackerId: packerID,
		PackedAt: timestamppb.Now(),
	}, nil
}

func (l *DefaultFulfillmentLogic) HandleShip(state *FulfillmentState, carrier, trackingNumber string) (*examples.Shipped, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsPacking() {
		return nil, NewFailedPrecondition(ErrMsgNotPacked)
	}
	if carrier == "" {
		return nil, NewInvalidArgument(ErrMsgCarrierRequired)
	}
	if trackingNumber == "" {
		return nil, NewInvalidArgument(ErrMsgTrackingNumRequired)
	}

	return &examples.Shipped{
		Carrier:        carrier,
		TrackingNumber: trackingNumber,
		ShippedAt:      timestamppb.Now(),
	}, nil
}

func (l *DefaultFulfillmentLogic) HandleRecordDelivery(state *FulfillmentState, signature string) (*examples.Delivered, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsShipped() {
		return nil, NewFailedPrecondition(ErrMsgNotShipped)
	}

	return &examples.Delivered{
		Signature:   signature,
		DeliveredAt: timestamppb.Now(),
	}, nil
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

func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
