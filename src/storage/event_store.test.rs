//! Tests for event store value objects.
//!
//! These are pure data structures with simple methods - no async, no I/O.

use super::{AddOutcome, SourceInfo};
use uuid::Uuid;

// ============================================================================
// SourceInfo Tests
// ============================================================================

/// SourceInfo::new creates with all fields.
#[test]
fn source_info_new_sets_all_fields() {
    let root = Uuid::new_v4();
    let info = SourceInfo::new("angzarr", "orders", root, 42);

    assert_eq!(info.edition, "angzarr");
    assert_eq!(info.domain, "orders");
    assert_eq!(info.root, root);
    assert_eq!(info.seq, 42);
}

/// SourceInfo::new accepts Into<String> for edition and domain.
#[test]
fn source_info_new_accepts_into_string() {
    let root = Uuid::new_v4();
    let info = SourceInfo::new(String::from("v2"), String::from("inventory"), root, 1);

    assert_eq!(info.edition, "v2");
    assert_eq!(info.domain, "inventory");
}

/// SourceInfo::is_empty returns true when edition and domain are empty.
#[test]
fn source_info_is_empty_when_both_empty() {
    let info = SourceInfo::default();
    assert!(info.is_empty());
}

/// SourceInfo::is_empty returns false when edition is set.
#[test]
fn source_info_is_not_empty_when_edition_set() {
    let info = SourceInfo {
        edition: "angzarr".to_string(),
        ..Default::default()
    };
    assert!(!info.is_empty());
}

/// SourceInfo::is_empty returns false when domain is set.
#[test]
fn source_info_is_not_empty_when_domain_set() {
    let info = SourceInfo {
        domain: "orders".to_string(),
        ..Default::default()
    };
    assert!(!info.is_empty());
}

/// SourceInfo::is_empty returns false when both are set.
#[test]
fn source_info_is_not_empty_when_both_set() {
    let info = SourceInfo::new("angzarr", "orders", Uuid::new_v4(), 1);
    assert!(!info.is_empty());
}

// ============================================================================
// AddOutcome Tests
// ============================================================================

/// AddOutcome::Added is_added returns true.
#[test]
fn add_outcome_added_is_added() {
    let outcome = AddOutcome::Added {
        first_sequence: 1,
        last_sequence: 5,
    };
    assert!(outcome.is_added());
    assert!(!outcome.is_duplicate());
}

/// AddOutcome::Duplicate is_duplicate returns true.
#[test]
fn add_outcome_duplicate_is_duplicate() {
    let outcome = AddOutcome::Duplicate {
        first_sequence: 1,
        last_sequence: 5,
    };
    assert!(outcome.is_duplicate());
    assert!(!outcome.is_added());
}

/// AddOutcome::first_sequence returns first for Added.
#[test]
fn add_outcome_first_sequence_added() {
    let outcome = AddOutcome::Added {
        first_sequence: 10,
        last_sequence: 15,
    };
    assert_eq!(outcome.first_sequence(), 10);
}

/// AddOutcome::first_sequence returns first for Duplicate.
#[test]
fn add_outcome_first_sequence_duplicate() {
    let outcome = AddOutcome::Duplicate {
        first_sequence: 20,
        last_sequence: 25,
    };
    assert_eq!(outcome.first_sequence(), 20);
}

/// AddOutcome::last_sequence returns last for Added.
#[test]
fn add_outcome_last_sequence_added() {
    let outcome = AddOutcome::Added {
        first_sequence: 10,
        last_sequence: 15,
    };
    assert_eq!(outcome.last_sequence(), 15);
}

/// AddOutcome::last_sequence returns last for Duplicate.
#[test]
fn add_outcome_last_sequence_duplicate() {
    let outcome = AddOutcome::Duplicate {
        first_sequence: 20,
        last_sequence: 25,
    };
    assert_eq!(outcome.last_sequence(), 25);
}

/// AddOutcome equality works correctly.
#[test]
fn add_outcome_equality() {
    let a = AddOutcome::Added {
        first_sequence: 1,
        last_sequence: 5,
    };
    let b = AddOutcome::Added {
        first_sequence: 1,
        last_sequence: 5,
    };
    let c = AddOutcome::Duplicate {
        first_sequence: 1,
        last_sequence: 5,
    };

    assert_eq!(a, b);
    assert_ne!(a, c);
}
