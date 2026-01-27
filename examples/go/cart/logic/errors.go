package logic

import "fmt"

type StatusCode int

const (
	StatusInvalidArgument StatusCode = iota
	StatusFailedPrecondition
)

// Error message constants for cart domain.
const (
	ErrMsgCartExists           = "Cart already exists"
	ErrMsgCartNotFound         = "Cart does not exist"
	ErrMsgCartCheckedOut       = "Cart is already checked out"
	ErrMsgCartEmpty            = "Cart is empty"
	ErrMsgItemNotInCart        = "Item not in cart"
	ErrMsgQuantityPositive     = "Quantity must be positive"
	ErrMsgCouponAlreadyApplied = "Coupon already applied"
	ErrMsgUnknownCommand       = "Unknown command type"
	ErrMsgNoCommandPages       = "CommandBook has no pages"
	ErrMsgCustomerIDRequired   = "Customer ID is required"
	ErrMsgProductIDRequired    = "Product ID is required"
	ErrMsgCouponCodeRequired   = "Coupon code is required"
	ErrMsgPercentageRange      = "Percentage must be 0-100"
	ErrMsgFixedDiscountNeg     = "Fixed discount cannot be negative"
	ErrMsgInvalidCouponType    = "Invalid coupon type"
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
