//! PositionStore interface tests.
//!
//! These tests verify the contract of the PositionStore trait.
//! Each storage implementation should run these tests.

use angzarr::storage::PositionStore;

// =============================================================================
// PositionStore::get tests
// =============================================================================

pub async fn test_get_nonexistent<S: PositionStore>(store: &S) {
    let result = store
        .get("test_handler", "test_domain", "test", b"nonexistent")
        .await
        .expect("get should succeed");
    assert!(result.is_none(), "nonexistent position should be None");
}

// =============================================================================
// PositionStore::put tests
// =============================================================================

pub async fn test_put_and_get<S: PositionStore>(store: &S) {
    let handler = "test_pos_put_get";
    let domain = "test_domain";
    let root = b"root_001";

    store
        .put(handler, domain, "test", root, 42)
        .await
        .expect("put should succeed");

    let result = store
        .get(handler, domain, "test", root)
        .await
        .expect("get should succeed")
        .expect("position should exist");

    assert_eq!(result, 42, "should return stored sequence");
}

pub async fn test_put_update<S: PositionStore>(store: &S) {
    let handler = "test_pos_update";
    let domain = "test_domain";
    let root = b"root_002";

    store.put(handler, domain, "test", root, 10).await.unwrap();
    store.put(handler, domain, "test", root, 25).await.unwrap();

    let result = store
        .get(handler, domain, "test", root)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result, 25, "should return updated sequence");
}

pub async fn test_put_zero_sequence<S: PositionStore>(store: &S) {
    let handler = "test_pos_zero";
    let domain = "test_domain";
    let root = b"root_003";

    store.put(handler, domain, "test", root, 0).await.unwrap();

    let result = store
        .get(handler, domain, "test", root)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result, 0, "should store sequence 0");
}

// =============================================================================
// Isolation tests
// =============================================================================

pub async fn test_handler_isolation<S: PositionStore>(store: &S) {
    let domain = "test_domain";
    let root = b"root_iso_handler";

    store
        .put("handler_a", domain, "test", root, 10)
        .await
        .unwrap();
    store
        .put("handler_b", domain, "test", root, 20)
        .await
        .unwrap();

    let a = store
        .get("handler_a", domain, "test", root)
        .await
        .unwrap()
        .unwrap();
    let b = store
        .get("handler_b", domain, "test", root)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(a, 10, "handler_a should be 10");
    assert_eq!(b, 20, "handler_b should be 20");
}

pub async fn test_domain_isolation<S: PositionStore>(store: &S) {
    let handler = "test_pos_dom_iso";
    let root = b"root_iso_domain";

    store
        .put(handler, "domain_x", "test", root, 5)
        .await
        .unwrap();
    store
        .put(handler, "domain_y", "test", root, 15)
        .await
        .unwrap();

    let x = store
        .get(handler, "domain_x", "test", root)
        .await
        .unwrap()
        .unwrap();
    let y = store
        .get(handler, "domain_y", "test", root)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(x, 5, "domain_x should be 5");
    assert_eq!(y, 15, "domain_y should be 15");
}

pub async fn test_root_isolation<S: PositionStore>(store: &S) {
    let handler = "test_pos_root_iso";
    let domain = "test_domain";

    store
        .put(handler, domain, "test", b"root_a", 100)
        .await
        .unwrap();
    store
        .put(handler, domain, "test", b"root_b", 200)
        .await
        .unwrap();

    let a = store
        .get(handler, domain, "test", b"root_a")
        .await
        .unwrap()
        .unwrap();
    let b = store
        .get(handler, domain, "test", b"root_b")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(a, 100, "root_a should be 100");
    assert_eq!(b, 200, "root_b should be 200");
}

pub async fn test_multiple_handlers_same_root<S: PositionStore>(store: &S) {
    let domain = "test_domain";
    let root = b"shared_root";

    for i in 0..5u32 {
        let handler = format!("test_handler_{}", i);
        store
            .put(&handler, domain, "test", root, i * 10)
            .await
            .unwrap();
    }

    for i in 0..5u32 {
        let handler = format!("test_handler_{}", i);
        let result = store
            .get(&handler, domain, "test", root)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result, i * 10, "handler_{} should be {}", i, i * 10);
    }
}

// =============================================================================
// Test runner macro
// =============================================================================

/// Run all PositionStore interface tests against a store implementation.
#[macro_export]
macro_rules! run_position_store_tests {
    ($store:expr) => {
        use $crate::storage::position_store_tests::*;

        // get tests
        test_get_nonexistent($store).await;
        println!("  test_get_nonexistent: PASSED");

        // put tests
        test_put_and_get($store).await;
        println!("  test_put_and_get: PASSED");

        test_put_update($store).await;
        println!("  test_put_update: PASSED");

        test_put_zero_sequence($store).await;
        println!("  test_put_zero_sequence: PASSED");

        // isolation tests
        test_handler_isolation($store).await;
        println!("  test_handler_isolation: PASSED");

        test_domain_isolation($store).await;
        println!("  test_domain_isolation: PASSED");

        test_root_isolation($store).await;
        println!("  test_root_isolation: PASSED");

        test_multiple_handlers_same_root($store).await;
        println!("  test_multiple_handlers_same_root: PASSED");
    };
}
