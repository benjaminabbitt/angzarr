package logic

type CartItem struct {
	ProductID      string
	Name           string
	Quantity       int32
	UnitPriceCents int32
}

type CartState struct {
	CustomerID    string
	Items         map[string]*CartItem // productID -> item
	SubtotalCents int32
	CouponCode    string
	DiscountCents int32
	Status        string // "active", "checked_out"
}

func (s *CartState) Exists() bool {
	return s.CustomerID != ""
}

func (s *CartState) IsActive() bool {
	return s.Status == "active"
}

func (s *CartState) CalculateSubtotal() int32 {
	var subtotal int32
	for _, item := range s.Items {
		subtotal += item.Quantity * item.UnitPriceCents
	}
	return subtotal
}

func EmptyState() *CartState {
	return &CartState{
		Items: make(map[string]*CartItem),
	}
}
