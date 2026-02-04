package logic

// InventoryState represents the current state of an inventory aggregate.
type InventoryState struct {
	ProductID         string
	OnHand            int32
	Reserved          int32
	LowStockThreshold int32
	Reservations      map[string]int32 // order_id -> quantity
}

// Exists returns true if the inventory has been initialized.
func (s *InventoryState) Exists() bool {
	return s.ProductID != ""
}

// Available returns the quantity available for reservation.
func (s *InventoryState) Available() int32 {
	return s.OnHand - s.Reserved
}

// IsLowStock returns true if available stock is below the threshold.
func (s *InventoryState) IsLowStock() bool {
	return s.Available() < s.LowStockThreshold
}
