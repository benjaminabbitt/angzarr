package logic

// Error message constants for inventory domain.
const (
	ErrMsgAlreadyInitialized  = "Inventory already initialized"
	ErrMsgNotInitialized      = "Inventory not initialized"
	ErrMsgProductIDRequired   = "Product ID is required"
	ErrMsgQuantityNegative    = "Quantity cannot be negative"
	ErrMsgQuantityPositive    = "Quantity must be positive"
	ErrMsgThresholdNegative   = "Low stock threshold cannot be negative"
	ErrMsgOrderIDRequired     = "Order ID is required"
	ErrMsgReservationExists   = "Reservation already exists for this order"
	ErrMsgReservationNotFound = "No reservation found for this order"
)
