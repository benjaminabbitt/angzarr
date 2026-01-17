// Package logic provides pure business logic for the product domain.
package logic

// ProductState represents the current state of a product aggregate.
type ProductState struct {
	SKU         string
	Name        string
	Description string
	PriceCents  int32
	Status      string // "active", "discontinued"
}

// Exists returns true if the product has been created.
func (s *ProductState) Exists() bool {
	return s.SKU != ""
}

// IsActive returns true if the product is active (not discontinued).
func (s *ProductState) IsActive() bool {
	return s.Status == "active"
}

// EmptyState returns an empty product state for new aggregates.
func EmptyState() *ProductState {
	return &ProductState{}
}
