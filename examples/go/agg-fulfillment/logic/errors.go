package logic

// Error message constants for fulfillment domain.
const (
	ErrMsgShipmentExists      = "Shipment already exists"
	ErrMsgShipmentNotFound    = "Shipment does not exist"
	ErrMsgNotPending          = "Shipment is not pending"
	ErrMsgNotPicked           = "Shipment is not picked"
	ErrMsgNotPacked           = "Shipment is not packed"
	ErrMsgNotShipped          = "Shipment is not shipped"
	ErrMsgAlreadyDelivered    = "Shipment is already delivered"
	ErrMsgOrderIDRequired     = "Order ID is required"
	ErrMsgPickerIDRequired    = "Picker ID is required"
	ErrMsgPackerIDRequired    = "Packer ID is required"
	ErrMsgCarrierRequired     = "Carrier is required"
	ErrMsgTrackingNumRequired = "Tracking number is required"
)
