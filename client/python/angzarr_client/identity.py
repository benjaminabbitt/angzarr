"""Aggregate identity computation for Angzarr domains.

Provides deterministic UUID generation from business keys, ensuring consistent
aggregate identification across services. Matches the Rust identity module.
"""

from __future__ import annotations

import uuid

# Namespace UUID for generating deterministic inventory product UUIDs.
# Matches the Rust INVENTORY_PRODUCT_NAMESPACE constant (uuid.NAMESPACE_DNS).
INVENTORY_PRODUCT_NAMESPACE = uuid.UUID("6ba7b810-9dad-11d1-80b4-00c04fd430c8")


def compute_root(domain: str, business_key: str) -> uuid.UUID:
    """Compute a deterministic root UUID from domain and business key.

    The UUID is derived from: hash("angzarr" + domain + business_key)
    using the OID namespace, matching the Rust compute_root function.
    """
    seed = f"angzarr{domain}{business_key}"
    return uuid.uuid5(uuid.NAMESPACE_OID, seed)


def inventory_product_root(product_id: str) -> uuid.UUID:
    """Generate a deterministic UUID for an inventory product aggregate.

    Uses UUID v5 with INVENTORY_PRODUCT_NAMESPACE to ensure the same product_id
    always maps to the same inventory aggregate root.
    """
    return uuid.uuid5(INVENTORY_PRODUCT_NAMESPACE, product_id)


def customer_root(email: str) -> uuid.UUID:
    """Compute a deterministic root UUID for a customer aggregate."""
    return compute_root("customer", email)


def product_root(sku: str) -> uuid.UUID:
    """Compute a deterministic root UUID for a product aggregate."""
    return compute_root("product", sku)


def order_root(order_id: str) -> uuid.UUID:
    """Compute a deterministic root UUID for an order aggregate."""
    return compute_root("order", order_id)


def inventory_root(product_id: str) -> uuid.UUID:
    """Compute a deterministic root UUID for an inventory aggregate."""
    return compute_root("inventory", product_id)


def cart_root(customer_id: str) -> uuid.UUID:
    """Compute a deterministic root UUID for a cart aggregate."""
    return compute_root("cart", customer_id)


def fulfillment_root(order_id: str) -> uuid.UUID:
    """Compute a deterministic root UUID for a fulfillment aggregate."""
    return compute_root("fulfillment", order_id)


def to_proto_bytes(id: uuid.UUID) -> bytes:
    """Convert a uuid.UUID to 16-byte proto representation."""
    return id.bytes
