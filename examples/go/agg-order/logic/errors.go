package logic

// Error message constants for order domain.
const (
	ErrMsgOrderExists         = "Order already exists"
	ErrMsgOrderNotFound       = "Order does not exist"
	ErrMsgCustomerIDRequired  = "Customer ID is required"
	ErrMsgItemsRequired       = "Order must have at least one item"
	ErrMsgItemQuantityPos     = "Item quantity must be positive"
	ErrMsgOrderNotPending     = "Order is not in pending state"
	ErrMsgCannotCancelDone    = "Cannot cancel completed order"
	ErrMsgAlreadyCancelled    = "Order already cancelled"
	ErrMsgReasonRequired      = "Cancellation reason is required"
	ErrMsgPaymentMethodReq    = "Payment method is required"
	ErrMsgPaymentAmountMatch  = "Payment amount must match order total"
	ErrMsgPaymentNotSubmitted = "Payment not submitted"
	ErrMsgPaymentRefRequired  = "Payment reference is required"
	ErrMsgPointsPositive      = "Points must be positive"
	ErrMsgDiscountPositive    = "Discount must be positive"
	ErrMsgDiscountExceeds     = "Discount cannot exceed subtotal"
	ErrMsgLoyaltyAlready      = "Loyalty discount already applied"
)
