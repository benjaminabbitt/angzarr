// Package logic provides pure business logic for the receipt projector.
// This package has no gRPC dependencies and can be tested in isolation.
package logic

import "projector-receipt/proto/examples"

// TransactionState holds the rebuilt state from transaction events.
type TransactionState struct {
	CustomerID          string
	Items               []*examples.LineItem
	SubtotalCents       int32
	DiscountCents       int32
	DiscountType        string
	FinalTotalCents     int32
	PaymentMethod       string
	LoyaltyPointsEarned int32
	Completed           bool
}

// EmptyTransactionState returns an empty transaction state for new projections.
func EmptyTransactionState() *TransactionState {
	return &TransactionState{}
}

// IsComplete returns true if the transaction has been completed.
func (s *TransactionState) IsComplete() bool {
	return s.Completed
}
