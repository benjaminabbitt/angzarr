"""Tests for identity module."""

import uuid

from angzarr_client.identity import (
    INVENTORY_PRODUCT_NAMESPACE,
    cart_root,
    compute_root,
    customer_root,
    fulfillment_root,
    inventory_product_root,
    inventory_root,
    order_root,
    product_root,
    to_proto_bytes,
)


class TestComputeRoot:
    def test_deterministic(self):
        root1 = compute_root("domain", "key@example.com")
        root2 = compute_root("domain", "key@example.com")
        assert root1 == root2

    def test_different_keys(self):
        root1 = compute_root("domain", "key1@example.com")
        root2 = compute_root("domain", "key2@example.com")
        assert root1 != root2

    def test_different_domains(self):
        root1 = compute_root("domain1", "test-123")
        root2 = compute_root("domain2", "test-123")
        assert root1 != root2

    def test_returns_uuid(self):
        root = compute_root("domain", "test")
        assert isinstance(root, uuid.UUID)
        assert root.version == 5


class TestDomainRoots:
    def test_customer_root(self):
        root = customer_root("key@example.com")
        expected = compute_root("customer", "key@example.com")
        assert root == expected

    def test_product_root(self):
        assert product_root("SKU-001") == compute_root("product", "SKU-001")

    def test_order_root(self):
        assert order_root("ID-123") == compute_root("order", "ID-123")

    def test_inventory_root(self):
        assert inventory_root("SKU-001") == compute_root("inventory", "SKU-001")

    def test_cart_root(self):
        assert cart_root("user-1") == compute_root("cart", "user-1")

    def test_fulfillment_root(self):
        assert fulfillment_root("ID-123") == compute_root("fulfillment", "ID-123")

    def test_all_domains_different(self):
        key = "test-key"
        roots = {
            customer_root(key),
            product_root(key),
            order_root(key),
            inventory_root(key),
            cart_root(key),
            fulfillment_root(key),
        }
        assert len(roots) == 6


class TestInventoryProductRoot:
    def test_deterministic(self):
        root1 = inventory_product_root("SKU-001")
        root2 = inventory_product_root("SKU-001")
        assert root1 == root2

    def test_different_products(self):
        root1 = inventory_product_root("SKU-001")
        root2 = inventory_product_root("SKU-002")
        assert root1 != root2

    def test_namespace_matches_rust(self):
        expected = uuid.UUID("6ba7b810-9dad-11d1-80b4-00c04fd430c8")
        assert INVENTORY_PRODUCT_NAMESPACE == expected


class TestToProtoBytes:
    def test_returns_16_bytes(self):
        id = uuid.uuid4()
        result = to_proto_bytes(id)
        assert len(result) == 16

    def test_round_trip(self):
        original = uuid.UUID("550e8400-e29b-41d4-a716-446655440000")
        result = to_proto_bytes(original)
        roundtrip = uuid.UUID(bytes=result)
        assert roundtrip == original
