//! Test support for merge module.
//!
//! This file is only compiled during tests via `#[path]` include in merge.rs.
//! Provides JSON-like state parsing for test aggregates, enabling commutative
//! merge testing without requiring proto reflection.

use std::collections::{HashMap, HashSet};

/// Parse test state JSON-like format into field -> value map.
///
/// Simple parsing of `{"field_a":100,"field_b":"hello"}`
pub(crate) fn parse_test_state_fields(s: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();

    let s = s.trim_start_matches('{').trim_end_matches('}');
    for part in s.split(',') {
        if let Some((key, val)) = part.split_once(':') {
            let key = key.trim().trim_matches('"');
            let val = val.trim();
            fields.insert(key.to_string(), val.to_string());
        }
    }

    fields
}

/// Diff test state fields using simple JSON-like parsing.
///
/// Returns set of field names that differ between before and after.
pub(crate) fn diff_test_state_fields(before: &[u8], after: &[u8]) -> HashSet<String> {
    let before_str = String::from_utf8_lossy(before);
    let after_str = String::from_utf8_lossy(after);

    let before_fields = parse_test_state_fields(&before_str);
    let after_fields = parse_test_state_fields(&after_str);

    let mut changed = HashSet::new();

    // Find fields that differ
    for (key, before_val) in &before_fields {
        match after_fields.get(key) {
            Some(after_val) if after_val != before_val => {
                changed.insert(key.clone());
            }
            None => {
                changed.insert(key.clone());
            }
            _ => {}
        }
    }

    // Find fields only in after
    for key in after_fields.keys() {
        if !before_fields.contains_key(key) {
            changed.insert(key.clone());
        }
    }

    changed
}
