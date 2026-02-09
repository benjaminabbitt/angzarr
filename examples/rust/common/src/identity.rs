//! Aggregate identity computation for Angzarr domains.
//!
//! Provides deterministic UUID generation from business keys, ensuring consistent
//! aggregate identification across services.

use uuid::Uuid;

/// Namespace UUID for generating deterministic inventory product UUIDs.
///
/// Used by saga-inventory-reservation and saga-checkout to derive consistent
/// inventory aggregate roots from product IDs.
pub const INVENTORY_PRODUCT_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Generate a deterministic UUID for an inventory product aggregate.
///
/// Uses UUID v5 with `INVENTORY_PRODUCT_NAMESPACE` to ensure the same product_id
/// always maps to the same inventory aggregate root.
pub fn inventory_product_root(product_id: &str) -> Uuid {
    Uuid::new_v5(&INVENTORY_PRODUCT_NAMESPACE, product_id.as_bytes())
}

/// Compute a deterministic root UUID from domain and business key.
///
/// The UUID is derived from: `hash("angzarr" + domain + business_key)`
///
/// This ensures:
/// - Same business key always produces same root
/// - Different domains with same key produce different roots
/// - Collisions at business key level are handled by Angzarr
pub fn compute_root(domain: &str, business_key: &str) -> Uuid {
    let seed = format!("angzarr{}{}", domain, business_key);
    Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes())
}

/// Compute root UUID for a customer aggregate.
///
/// Business key: email address
pub fn customer_root(email: &str) -> Uuid {
    compute_root("customer", email)
}

/// Compute root UUID for a product aggregate.
///
/// Business key: SKU
pub fn product_root(sku: &str) -> Uuid {
    compute_root("product", sku)
}

/// Compute root UUID for an order aggregate.
///
/// Business key: order ID (typically a generated identifier)
pub fn order_root(order_id: &str) -> Uuid {
    compute_root("order", order_id)
}

/// Compute root UUID for an inventory aggregate.
///
/// Business key: product ID
pub fn inventory_root(product_id: &str) -> Uuid {
    compute_root("inventory", product_id)
}

/// Compute root UUID for a cart aggregate.
///
/// Business key: customer ID (one cart per customer)
pub fn cart_root(customer_id: &str) -> Uuid {
    compute_root("cart", customer_id)
}

/// Compute root UUID for a fulfillment aggregate.
///
/// Business key: order ID
pub fn fulfillment_root(order_id: &str) -> Uuid {
    compute_root("fulfillment", order_id)
}

/// Compute root UUID for an accounting aggregate.
///
/// Business key: varies by record type (order_id or customer_id)
pub fn accounting_root(key: &str) -> Uuid {
    compute_root("accounting", key)
}

/// Compute root UUID for a web view aggregate.
///
/// Business key: varies by entity type (product_id, order_id, cart_id, customer_id)
pub fn web_root(key: &str) -> Uuid {
    compute_root("web", key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_root_deterministic() {
        let root1 = compute_root("customer", "alice@example.com");
        let root2 = compute_root("customer", "alice@example.com");
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_compute_root_different_keys() {
        let root1 = compute_root("customer", "alice@example.com");
        let root2 = compute_root("customer", "bob@example.com");
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_compute_root_different_domains() {
        let root1 = compute_root("customer", "test-123");
        let root2 = compute_root("order", "test-123");
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_customer_root() {
        let root = customer_root("alice@example.com");
        assert_eq!(root, compute_root("customer", "alice@example.com"));
    }

    #[test]
    fn test_inventory_product_root_deterministic() {
        let root1 = inventory_product_root("SKU-001");
        let root2 = inventory_product_root("SKU-001");
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_inventory_product_root_different_products() {
        let root1 = inventory_product_root("SKU-001");
        let root2 = inventory_product_root("SKU-002");
        assert_ne!(root1, root2);
    }
}
