//! Tests for subscription target parsing and matching.
//!
//! Subscription targets filter which events a component receives:
//! - Domain specifies the source bounded context
//! - Types list (optional) restricts to specific event types
//!
//! Why this matters: Sagas and PMs must declare subscriptions upfront.
//! Without filtering, every component would receive every event,
//! wasting resources and complicating routing logic.
//!
//! Format: "domain:Type1,Type2;domain2" parsed from env vars.
//! Empty types list means "all events from domain".
//!
//! Key behaviors verified:
//! - Empty types matches all event types
//! - Specific types use suffix matching (handles type URL prefixes)
//! - Parse handles mixed formats (with/without types)

use super::*;

// ============================================================================
// Target Matching Tests
// ============================================================================

/// Empty types list matches all events from domain.
///
/// Shorthand for "give me everything from this domain".
/// Used when sagas need all events regardless of type.
#[test]
fn test_target_matches_all() {
    let target = Target::domain("order");
    assert!(target.matches_type("OrderCreated"));
    assert!(target.matches_type("OrderShipped"));
    assert!(target.matches_type("anything"));
}

/// Specific types use suffix matching for full type URLs.
///
/// Type URLs include prefix: "type.googleapis.com/examples.OrderCreated"
/// Suffix matching lets config use short names: "OrderCreated"
#[test]
fn test_target_matches_specific() {
    let target = Target::new("order", vec!["OrderCreated", "OrderShipped"]);
    assert!(target.matches_type("type.googleapis.com/examples.OrderCreated"));
    assert!(target.matches_type("OrderShipped"));
    assert!(!target.matches_type("OrderCancelled"));
}

// ============================================================================
// Subscription Parsing Tests
// ============================================================================

/// Parse format with explicit types: "domain:Type1,Type2".
///
/// Most common format for selective subscriptions.
#[test]
fn test_parse_subscriptions_with_types() {
    let subs = parse_subscriptions("order:OrderCreated,OrderShipped;inventory:StockReserved");
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].domain, "order");
    assert_eq!(subs[0].types, vec!["OrderCreated", "OrderShipped"]);
    assert_eq!(subs[1].domain, "inventory");
    assert_eq!(subs[1].types, vec!["StockReserved"]);
}

/// Parse domain-only format (no colon): matches all types.
///
/// Shorthand when you need every event from a domain.
#[test]
fn test_parse_subscriptions_all_types() {
    let subs = parse_subscriptions("order;inventory");
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].domain, "order");
    assert!(subs[0].types.is_empty());
    assert_eq!(subs[1].domain, "inventory");
    assert!(subs[1].types.is_empty());
}

/// Empty string produces empty subscription list.
///
/// No subscriptions = component receives no events.
#[test]
fn test_parse_subscriptions_empty() {
    let subs = parse_subscriptions("");
    assert!(subs.is_empty());
}

/// Mixed format: some domains with types, some without.
#[test]
fn test_parse_subscriptions_mixed() {
    let subs = parse_subscriptions("order:OrderCreated;inventory");
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].types, vec!["OrderCreated"]);
    assert!(subs[1].types.is_empty());
}
