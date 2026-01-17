package logic

type FulfillmentState struct {
	OrderID        string
	Status         string // "pending", "picking", "packing", "shipped", "delivered"
	TrackingNumber string
	Carrier        string
	PickerID       string
	PackerID       string
	Signature      string
}

func (s *FulfillmentState) Exists() bool {
	return s.OrderID != ""
}

func (s *FulfillmentState) IsPending() bool {
	return s.Status == "pending"
}

func (s *FulfillmentState) IsPicking() bool {
	return s.Status == "picking"
}

func (s *FulfillmentState) IsPacking() bool {
	return s.Status == "packing"
}

func (s *FulfillmentState) IsShipped() bool {
	return s.Status == "shipped"
}

func (s *FulfillmentState) IsDelivered() bool {
	return s.Status == "delivered"
}

func EmptyState() *FulfillmentState {
	return &FulfillmentState{}
}
