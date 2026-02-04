package angzarr

import (
	"testing"

	"github.com/google/uuid"
)

func TestComputeRootDeterministic(t *testing.T) {
	root1 := ComputeRoot("customer", "alice@example.com")
	root2 := ComputeRoot("customer", "alice@example.com")

	if root1 != root2 {
		t.Errorf("same inputs should produce same root: %s != %s", root1, root2)
	}
}

func TestComputeRootDifferentKeys(t *testing.T) {
	root1 := ComputeRoot("customer", "alice@example.com")
	root2 := ComputeRoot("customer", "bob@example.com")

	if root1 == root2 {
		t.Error("different keys should produce different roots")
	}
}

func TestComputeRootDifferentDomains(t *testing.T) {
	root1 := ComputeRoot("customer", "test-123")
	root2 := ComputeRoot("order", "test-123")

	if root1 == root2 {
		t.Error("different domains should produce different roots")
	}
}

func TestCustomerRoot(t *testing.T) {
	root := CustomerRoot("alice@example.com")
	expected := ComputeRoot("customer", "alice@example.com")

	if root != expected {
		t.Errorf("CustomerRoot mismatch: %s != %s", root, expected)
	}
}

func TestInventoryProductRootDeterministic(t *testing.T) {
	root1 := InventoryProductRoot("SKU-001")
	root2 := InventoryProductRoot("SKU-001")

	if root1 != root2 {
		t.Errorf("same product_id should produce same root: %s != %s", root1, root2)
	}
}

func TestInventoryProductRootDifferentProducts(t *testing.T) {
	root1 := InventoryProductRoot("SKU-001")
	root2 := InventoryProductRoot("SKU-002")

	if root1 == root2 {
		t.Error("different product IDs should produce different roots")
	}
}

func TestToProtoUUID(t *testing.T) {
	id := uuid.MustParse("550e8400-e29b-41d4-a716-446655440000")
	proto := ToProtoUUID(id)

	if len(proto.Value) != 16 {
		t.Errorf("proto UUID should be 16 bytes, got %d", len(proto.Value))
	}

	// Round-trip: bytes back to UUID
	roundTrip, err := uuid.FromBytes(proto.Value)
	if err != nil {
		t.Fatalf("failed to parse proto UUID bytes: %v", err)
	}
	if roundTrip != id {
		t.Errorf("round-trip mismatch: %s != %s", roundTrip, id)
	}
}

func TestAllDomainRootsAreDifferent(t *testing.T) {
	key := "test-key"
	roots := map[string]uuid.UUID{
		"customer":    CustomerRoot(key),
		"product":     ProductRoot(key),
		"order":       OrderRoot(key),
		"inventory":   InventoryRoot(key),
		"cart":        CartRoot(key),
		"fulfillment": FulfillmentRoot(key),
	}

	seen := make(map[uuid.UUID]string)
	for domain, root := range roots {
		if prev, exists := seen[root]; exists {
			t.Errorf("domain %q and %q produced same root for key %q", domain, prev, key)
		}
		seen[root] = domain
	}
}

func TestInventoryProductNamespaceMatchesRust(t *testing.T) {
	// Rust INVENTORY_PRODUCT_NAMESPACE is the DNS namespace UUID
	expected := uuid.MustParse("6ba7b810-9dad-11d1-80b4-00c04fd430c8")
	if InventoryProductNamespace != expected {
		t.Errorf("namespace mismatch: %s != %s", InventoryProductNamespace, expected)
	}
}
