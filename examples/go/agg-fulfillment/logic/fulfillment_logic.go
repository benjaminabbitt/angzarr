package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	goproto "google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// ============================================================================
// Named event appliers
// ============================================================================

func applyShipmentCreated(state *FulfillmentState, event *anypb.Any) {
	var e examples.ShipmentCreated
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.OrderID = e.OrderId
	state.Status = FulfillmentStatus(e.Status)
	state.Items = e.Items
}

func applyItemsPicked(state *FulfillmentState, event *anypb.Any) {
	var e examples.ItemsPicked
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.PickerID = e.PickerId
	state.Status = FulfillmentStatusPicking
}

func applyItemsPacked(state *FulfillmentState, event *anypb.Any) {
	var e examples.ItemsPacked
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.PackerID = e.PackerId
	state.Status = FulfillmentStatusPacking
}

func applyShipped(state *FulfillmentState, event *anypb.Any) {
	var e examples.Shipped
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.Carrier = e.Carrier
	state.TrackingNumber = e.TrackingNumber
	state.Status = FulfillmentStatusShipped
}

func applyDelivered(state *FulfillmentState, event *anypb.Any) {
	var e examples.Delivered
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.Signature = e.Signature
	state.Status = FulfillmentStatusDelivered
}

func loadFulfillmentSnapshot(state *FulfillmentState, snapshot *anypb.Any) {
	var snapState examples.FulfillmentState
	if err := snapshot.UnmarshalTo(&snapState); err != nil {
		return
	}
	state.OrderID = snapState.OrderId
	state.Status = FulfillmentStatus(snapState.Status)
	state.TrackingNumber = snapState.TrackingNumber
	state.Carrier = snapState.Carrier
	state.PickerID = snapState.PickerId
	state.PackerID = snapState.PackerId
	state.Signature = snapState.Signature
	state.Items = snapState.Items
}

// ============================================================================
// State rebuilding
// ============================================================================

// stateBuilder is the single source of truth for event type → applier mapping.
var stateBuilder = angzarr.NewStateBuilder(func() FulfillmentState { return FulfillmentState{} }).
	WithSnapshot(loadFulfillmentSnapshot).
	On(angzarr.Name(&examples.ShipmentCreated{}), applyShipmentCreated).
	On(angzarr.Name(&examples.ItemsPicked{}), applyItemsPicked).
	On(angzarr.Name(&examples.ItemsPacked{}), applyItemsPacked).
	On(angzarr.Name(&examples.Shipped{}), applyShipped).
	On(angzarr.Name(&examples.Delivered{}), applyDelivered)

// RebuildState reconstructs fulfillment state from an event book.
func RebuildState(eventBook *angzarrpb.EventBook) FulfillmentState {
	return stateBuilder.Rebuild(eventBook)
}

// ApplyEvent applies a single event to the fulfillment state.
func ApplyEvent(state *FulfillmentState, event *anypb.Any) {
	stateBuilder.Apply(state, event)
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
		Status:    FulfillmentStatusPending.String(),
		CreatedAt: timestamppb.Now(),
		Items:     cmd.Items,
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
		Items:          state.Items,
		OrderId:        state.OrderID,
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
