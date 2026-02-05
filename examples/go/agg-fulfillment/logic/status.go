package logic

// FulfillmentStatus represents the status of a fulfillment in the aggregate.
type FulfillmentStatus string

const (
	// FulfillmentStatusPending indicates the shipment is pending.
	FulfillmentStatusPending FulfillmentStatus = "pending"
	// FulfillmentStatusPicking indicates items are being picked.
	FulfillmentStatusPicking FulfillmentStatus = "picking"
	// FulfillmentStatusPacking indicates items are being packed.
	FulfillmentStatusPacking FulfillmentStatus = "packing"
	// FulfillmentStatusShipped indicates the shipment has been shipped.
	FulfillmentStatusShipped FulfillmentStatus = "shipped"
	// FulfillmentStatusDelivered indicates the shipment has been delivered.
	FulfillmentStatusDelivered FulfillmentStatus = "delivered"
)

// String returns the string representation of the status.
func (s FulfillmentStatus) String() string {
	return string(s)
}
