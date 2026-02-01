// Package logic provides pure client logic for the customer domain.
// This package has no gRPC dependencies and can be tested in isolation.
package logic

// CustomerState represents the current state of a customer aggregate.
type CustomerState struct {
	Name           string
	Email          string
	LoyaltyPoints  int32
	LifetimePoints int32
}

// Exists returns true if the customer has been created.
func (s *CustomerState) Exists() bool {
	return s.Name != ""
}

// EmptyState returns an empty customer state for new aggregates.
func EmptyState() *CustomerState {
	return &CustomerState{}
}
