package logic

import "angzarr/proto/examples"

// FulfillmentState represents the current state of a fulfillment aggregate.
type FulfillmentState struct {
	OrderID        string
	Status         FulfillmentStatus
	TrackingNumber string
	Carrier        string
	PickerID       string
	PackerID       string
	Signature      string
	Items          []*examples.LineItem
}

// Exists returns true if the shipment has been created.
func (s *FulfillmentState) Exists() bool {
	return s.OrderID != ""
}

// IsPending returns true if the shipment is pending.
func (s *FulfillmentState) IsPending() bool {
	return s.Status == FulfillmentStatusPending
}

// IsPicking returns true if the shipment is in picking status.
func (s *FulfillmentState) IsPicking() bool {
	return s.Status == FulfillmentStatusPicking
}

// IsPacking returns true if the shipment is in packing status.
func (s *FulfillmentState) IsPacking() bool {
	return s.Status == FulfillmentStatusPacking
}

// IsShipped returns true if the shipment has been shipped.
func (s *FulfillmentState) IsShipped() bool {
	return s.Status == FulfillmentStatusShipped
}

// IsDelivered returns true if the shipment has been delivered.
func (s *FulfillmentState) IsDelivered() bool {
	return s.Status == FulfillmentStatusDelivered
}
