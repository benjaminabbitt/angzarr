// Package logic provides pure business logic for the transaction domain.
// This package has no gRPC dependencies and can be tested in isolation.
package logic

import "transaction/proto/examples"

// TransactionState represents the current state of a transaction aggregate.
type TransactionState struct {
	CustomerID    string
	Items         []*examples.LineItem
	SubtotalCents int32
	DiscountCents int32
	DiscountType  string
	Status        string // "new", "pending", "completed", "cancelled"
}

// Exists returns true if the transaction has been created.
func (s *TransactionState) Exists() bool {
	return s.Status != "new"
}

// IsPending returns true if the transaction is in pending state.
func (s *TransactionState) IsPending() bool {
	return s.Status == "pending"
}

// IsCompleted returns true if the transaction is completed.
func (s *TransactionState) IsCompleted() bool {
	return s.Status == "completed"
}

// IsCancelled returns true if the transaction is cancelled.
func (s *TransactionState) IsCancelled() bool {
	return s.Status == "cancelled"
}

// EmptyState returns an empty transaction state for new aggregates.
func EmptyState() *TransactionState {
	return &TransactionState{
		Status: "new",
	}
}
