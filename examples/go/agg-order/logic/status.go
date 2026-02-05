package logic

// OrderStatus represents the status of an order in the aggregate.
type OrderStatus string

const (
	// OrderStatusPending indicates the order has been created but not yet paid.
	OrderStatusPending OrderStatus = "pending"
	// OrderStatusPaymentSubmitted indicates payment has been submitted.
	OrderStatusPaymentSubmitted OrderStatus = "payment_submitted"
	// OrderStatusCompleted indicates the order is fully processed.
	OrderStatusCompleted OrderStatus = "completed"
	// OrderStatusCancelled indicates the order was cancelled.
	OrderStatusCancelled OrderStatus = "cancelled"
)

// String returns the string representation of the status.
func (s OrderStatus) String() string {
	return string(s)
}
