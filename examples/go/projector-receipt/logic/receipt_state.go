// Package logic provides pure business logic for the receipt projector.
// This package has no gRPC dependencies and can be tested in isolation.
package logic

import "projector-receipt/proto/examples"

// OrderState holds the rebuilt state from order events.
type OrderState struct {
	CustomerID          string
	Items               []*examples.LineItem
	SubtotalCents       int32
	DiscountCents       int32
	LoyaltyPointsUsed   int32
	FinalTotalCents     int32
	PaymentMethod       string
	PaymentReference    string
	LoyaltyPointsEarned int32
	Completed           bool
}

// EmptyOrderState returns an empty order state for new projections.
func EmptyOrderState() *OrderState {
	return &OrderState{}
}

// IsComplete returns true if the order has been completed.
func (s *OrderState) IsComplete() bool {
	return s.Completed
}
