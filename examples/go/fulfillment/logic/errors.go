package logic

import "fmt"

type StatusCode int

const (
	StatusInvalidArgument StatusCode = iota
	StatusFailedPrecondition
)

// Error message constants for fulfillment domain.
const (
	ErrMsgShipmentExists       = "Shipment already exists"
	ErrMsgShipmentNotFound     = "Shipment does not exist"
	ErrMsgNotPending           = "Shipment is not pending"
	ErrMsgNotPicked            = "Shipment is not picked"
	ErrMsgNotPacked            = "Shipment is not packed"
	ErrMsgNotShipped           = "Shipment is not shipped"
	ErrMsgAlreadyDelivered     = "Shipment is already delivered"
	ErrMsgUnknownCommand       = "Unknown command type"
	ErrMsgNoCommandPages       = "CommandBook has no pages"
	ErrMsgOrderIDRequired      = "Order ID is required"
	ErrMsgPickerIDRequired     = "Picker ID is required"
	ErrMsgPackerIDRequired     = "Packer ID is required"
	ErrMsgCarrierRequired      = "Carrier is required"
	ErrMsgTrackingNumRequired  = "Tracking number is required"
)

func (s StatusCode) String() string {
	switch s {
	case StatusInvalidArgument:
		return "INVALID_ARGUMENT"
	case StatusFailedPrecondition:
		return "FAILED_PRECONDITION"
	default:
		return "UNKNOWN"
	}
}

type CommandError struct {
	Code    StatusCode
	Message string
}

func (e *CommandError) Error() string {
	return e.Message
}

func NewInvalidArgument(message string) *CommandError {
	return &CommandError{Code: StatusInvalidArgument, Message: message}
}

func NewFailedPrecondition(message string) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: message}
}

func NewFailedPreconditionf(format string, args ...interface{}) *CommandError {
	return &CommandError{Code: StatusFailedPrecondition, Message: fmt.Sprintf(format, args...)}
}
