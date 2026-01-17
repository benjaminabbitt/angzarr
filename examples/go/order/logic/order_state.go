package logic

type LineItem struct {
	ProductID      string
	Name           string
	Quantity       int32
	UnitPriceCents int32
}

type OrderState struct {
	CustomerID        string
	Items             []LineItem
	SubtotalCents     int32
	DiscountCents     int32
	LoyaltyPointsUsed int32
	PaymentMethod     string
	PaymentReference  string
	Status            string // "pending", "payment_submitted", "completed", "cancelled"
}

func (s *OrderState) Exists() bool {
	return s.CustomerID != ""
}

func (s *OrderState) IsPending() bool {
	return s.Status == "pending"
}

func (s *OrderState) IsPaymentSubmitted() bool {
	return s.Status == "payment_submitted"
}

func (s *OrderState) IsCompleted() bool {
	return s.Status == "completed"
}

func (s *OrderState) IsCancelled() bool {
	return s.Status == "cancelled"
}

func (s *OrderState) TotalAfterDiscount() int32 {
	return s.SubtotalCents - s.DiscountCents
}

func EmptyState() *OrderState {
	return &OrderState{
		Items: make([]LineItem, 0),
	}
}
