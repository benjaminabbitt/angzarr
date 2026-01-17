package logic

type InventoryState struct {
	ProductID         string
	OnHand            int32
	Reserved          int32
	LowStockThreshold int32
	Reservations      map[string]int32 // order_id -> quantity
}

func (s *InventoryState) Exists() bool {
	return s.ProductID != ""
}

func (s *InventoryState) Available() int32 {
	return s.OnHand - s.Reserved
}

func (s *InventoryState) IsLowStock() bool {
	return s.Available() < s.LowStockThreshold
}

func EmptyState() *InventoryState {
	return &InventoryState{
		Reservations: make(map[string]int32),
	}
}
