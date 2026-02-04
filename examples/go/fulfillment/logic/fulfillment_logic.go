package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"fulfillment/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// RebuildState reconstructs fulfillment state from an event book.
func RebuildState(eventBook *angzarrpb.EventBook) FulfillmentState {
	state := FulfillmentState{}

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

// HandleCreateShipment validates and creates a ShipmentCreated event.
func HandleCreateShipment(cb *angzarrpb.CommandBook, data []byte, state *FulfillmentState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.CreateShipment
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgShipmentExists)
	}
	if cmd.OrderId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgOrderIDRequired)
	}

	return angzarr.PackEvent(cb.Cover, &examples.ShipmentCreated{
		OrderId:   cmd.OrderId,
		Status:    "pending",
		CreatedAt: timestamppb.Now(),
	}, seq)
}

// HandleMarkPicked validates and creates an ItemsPicked event.
func HandleMarkPicked(cb *angzarrpb.CommandBook, data []byte, state *FulfillmentState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.MarkPicked
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsPending() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotPending)
	}
	if cmd.PickerId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgPickerIDRequired)
	}

	return angzarr.PackEvent(cb.Cover, &examples.ItemsPicked{
		PickerId: cmd.PickerId,
		PickedAt: timestamppb.Now(),
	}, seq)
}

// HandleMarkPacked validates and creates an ItemsPacked event.
func HandleMarkPacked(cb *angzarrpb.CommandBook, data []byte, state *FulfillmentState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.MarkPacked
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsPicking() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotPicked)
	}
	if cmd.PackerId == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgPackerIDRequired)
	}

	return angzarr.PackEvent(cb.Cover, &examples.ItemsPacked{
		PackerId: cmd.PackerId,
		PackedAt: timestamppb.Now(),
	}, seq)
}

// HandleShip validates and creates a Shipped event.
func HandleShip(cb *angzarrpb.CommandBook, data []byte, state *FulfillmentState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.Ship
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsPacking() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotPacked)
	}
	if cmd.Carrier == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgCarrierRequired)
	}
	if cmd.TrackingNumber == "" {
		return nil, angzarr.NewInvalidArgument(ErrMsgTrackingNumRequired)
	}

	return angzarr.PackEvent(cb.Cover, &examples.Shipped{
		Carrier:        cmd.Carrier,
		TrackingNumber: cmd.TrackingNumber,
		ShippedAt:      timestamppb.Now(),
	}, seq)
}

// HandleRecordDelivery validates and creates a Delivered event.
func HandleRecordDelivery(cb *angzarrpb.CommandBook, data []byte, state *FulfillmentState, seq uint32) (*angzarrpb.EventBook, error) {
	var cmd examples.RecordDelivery
	if err := goproto.Unmarshal(data, &cmd); err != nil {
		return nil, angzarr.NewInvalidArgument("failed to unmarshal command: " + err.Error())
	}

	if !state.Exists() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgShipmentNotFound)
	}
	if !state.IsShipped() {
		return nil, angzarr.NewFailedPrecondition(ErrMsgNotShipped)
	}

	return angzarr.PackEvent(cb.Cover, &examples.Delivered{
		Signature:   cmd.Signature,
		DeliveredAt: timestamppb.Now(),
	}, seq)
}
