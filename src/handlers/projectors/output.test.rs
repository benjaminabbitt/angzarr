//! Tests for event colorization and output formatting.
//!
//! The output system classifies events by category (Success, Progress,
//! Info, Failure) and applies ANSI colors for terminal display.
//! Classification can be explicit (type name mapping) or pattern-based
//! (e.g., "Created" suffix -> Success).
//!
//! Correct classification improves log readability - failures stand out
//! in red, successes in green, etc.

use super::*;

// ============================================================================
// Color Configuration Tests
// ============================================================================

/// Exact type name match takes priority over patterns.
///
/// When both an explicit mapping and pattern would match, the explicit
/// mapping wins. This allows overriding default classifications.
#[test]
fn test_event_color_config_exact_match() {
    let config = EventColorConfig::new()
        .with("examples.PlayerRegistered", EventCategory::Success)
        .with("OrderCancelled", EventCategory::Failure);

    assert_eq!(
        config.get_category("examples.PlayerRegistered"),
        EventCategory::Success
    );
    assert_eq!(
        config.get_category("OrderCancelled"),
        EventCategory::Failure
    );
    assert_eq!(config.get_category("UnknownEvent"), EventCategory::Default);
}

/// Simple name extracted from fully-qualified type URL.
///
/// Type URLs like "examples.PlayerRegistered" should match config
/// entries for just "PlayerRegistered". Simplifies configuration.
#[test]
fn test_event_color_config_simple_name_match() {
    let config = EventColorConfig::new().with("PlayerRegistered", EventCategory::Success);

    // Should match simple name extracted from full name
    assert_eq!(
        config.get_category("examples.PlayerRegistered"),
        EventCategory::Success
    );
}

/// Default patterns classify events by suffix.
///
/// Convention-based classification:
/// - "Created", "Completed" -> Success (green)
/// - "Cancelled", "Failed", "Rejected" -> Failure (red)
/// - "Added", "Updated", "Applied" -> Progress (yellow)
/// - "Started", "Initiated" -> Info (cyan)
#[test]
fn test_event_color_config_default_patterns() {
    let config = EventColorConfig::with_default_patterns();

    assert_eq!(
        config.get_category("examples.OrderCreated"),
        EventCategory::Success
    );
    assert_eq!(
        config.get_category("examples.OrderCompleted"),
        EventCategory::Success
    );
    assert_eq!(
        config.get_category("examples.OrderCancelled"),
        EventCategory::Failure
    );
    assert_eq!(
        config.get_category("examples.ItemAdded"),
        EventCategory::Progress
    );
    assert_eq!(
        config.get_category("examples.ProcessStarted"),
        EventCategory::Info
    );
}

// ============================================================================
// Output Implementation Tests
// ============================================================================

/// Stdout output handles event without panic.
///
/// Smoke test for stdout output - verifies basic operation without
/// crashing on typical input.
#[test]
fn test_stdout_output() {
    let output = StdoutOutput;
    let event = DecodedEvent {
        domain: "test",
        root_id: "abc123",
        sequence: 1,
        type_name: "TestEvent",
        content: "{ \"key\": \"value\" }",
    };

    // Just verify it doesn't panic
    output.write_event(&event);
}
