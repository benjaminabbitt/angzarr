package angzarr

import (
	angzarrpb "angzarr/proto/angzarr"

	"github.com/google/uuid"
)

// InventoryProductNamespace is the UUID namespace for generating deterministic
// inventory product UUIDs. Matches the Rust INVENTORY_PRODUCT_NAMESPACE constant.
var InventoryProductNamespace = uuid.MustParse("6ba7b810-9dad-11d1-80b4-00c04fd430c8")

// ComputeRoot derives a deterministic UUID v5 from a domain and business key.
//
// The UUID is derived from: hash("angzarr" + domain + business_key)
// using the OID namespace, matching the Rust compute_root function.
func ComputeRoot(domain, businessKey string) uuid.UUID {
	seed := "angzarr" + domain + businessKey
	return uuid.NewSHA1(uuid.NameSpaceOID, []byte(seed))
}

// InventoryProductRoot generates a deterministic UUID for an inventory product aggregate.
//
// Uses UUID v5 with InventoryProductNamespace to ensure the same product_id
// always maps to the same inventory aggregate root.
func InventoryProductRoot(productID string) uuid.UUID {
	return uuid.NewSHA1(InventoryProductNamespace, []byte(productID))
}

// CustomerRoot computes a deterministic root UUID for a customer aggregate.
func CustomerRoot(email string) uuid.UUID {
	return ComputeRoot("customer", email)
}

// ProductRoot computes a deterministic root UUID for a product aggregate.
func ProductRoot(sku string) uuid.UUID {
	return ComputeRoot("product", sku)
}

// OrderRoot computes a deterministic root UUID for an order aggregate.
func OrderRoot(orderID string) uuid.UUID {
	return ComputeRoot("order", orderID)
}

// InventoryRoot computes a deterministic root UUID for an inventory aggregate.
func InventoryRoot(productID string) uuid.UUID {
	return ComputeRoot("inventory", productID)
}

// CartRoot computes a deterministic root UUID for a cart aggregate.
func CartRoot(customerID string) uuid.UUID {
	return ComputeRoot("cart", customerID)
}

// FulfillmentRoot computes a deterministic root UUID for a fulfillment aggregate.
func FulfillmentRoot(orderID string) uuid.UUID {
	return ComputeRoot("fulfillment", orderID)
}

// ToProtoUUID converts a uuid.UUID to an angzarr proto UUID.
func ToProtoUUID(id uuid.UUID) *angzarrpb.UUID {
	bytes := id[:]
	return &angzarrpb.UUID{Value: bytes}
}
