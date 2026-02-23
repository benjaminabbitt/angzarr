//! Log output abstraction for the logging projector.
//!
//! Provides a trait-based output system allowing different output destinations
//! (stdout, files) and decorators (colorization).
//!
//! # Example
//!
//! ```rust,ignore
//! use angzarr::handlers::projectors::output::{
//!     StdoutOutput, FileOutput, ColorizingOutput, EventColorConfig, LogOutput
//! };
//!
//! // Plain stdout
//! let output = StdoutOutput;
//!
//! // Colored stdout
//! let config = EventColorConfig::default();
//! let output = ColorizingOutput::new(StdoutOutput, config);
//!
//! // File output
//! let output = FileOutput::new("events.log")?;
//!
//! // Colored file output
//! let output = ColorizingOutput::new(FileOutput::new("events.log")?, config);
//! ```

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

/// Decoded event data passed to output implementations.
#[derive(Debug, Clone)]
pub struct DecodedEvent<'a> {
    /// Domain the event belongs to.
    pub domain: &'a str,
    /// Root aggregate ID (hex-encoded).
    pub root_id: &'a str,
    /// Event sequence number.
    pub sequence: u32,
    /// Full type name from reflection (e.g., "examples.PlayerRegistered").
    pub type_name: &'a str,
    /// JSON-decoded content or hex dump.
    pub content: &'a str,
}

/// Abstraction for log output destinations.
pub trait LogOutput: Send + Sync {
    /// Write an event to the output.
    fn write_event(&self, event: &DecodedEvent);
}

// ANSI color codes for terminal output
const BLUE: &str = "\x1b[94m";
const GREEN: &str = "\x1b[92m";
const YELLOW: &str = "\x1b[93m";
const CYAN: &str = "\x1b[96m";
const RED: &str = "\x1b[91m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Event category for colorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventCategory {
    /// Successful completion events (green).
    Success,
    /// Progress/update events (yellow).
    Progress,
    /// Informational events (cyan).
    Info,
    /// Failure/cancellation events (red).
    Failure,
    /// Default category (blue).
    #[default]
    Default,
}

impl EventCategory {
    /// Get ANSI color code for this category.
    pub fn color(&self) -> &'static str {
        match self {
            EventCategory::Success => GREEN,
            EventCategory::Progress => YELLOW,
            EventCategory::Info => CYAN,
            EventCategory::Failure => RED,
            EventCategory::Default => BLUE,
        }
    }
}

/// Configuration for event colorization.
///
/// Maps event type names to categories. Type names can be:
/// - Full names: "examples.PlayerRegistered"
/// - Simple names: "PlayerRegistered"
#[derive(Debug, Clone, Default)]
pub struct EventColorConfig {
    /// Map of type name to category.
    categories: HashMap<String, EventCategory>,
    /// Default patterns for fallback classification.
    use_default_patterns: bool,
}

impl EventColorConfig {
    /// Create a new empty config.
    pub fn new() -> Self {
        Self {
            categories: HashMap::new(),
            use_default_patterns: false,
        }
    }

    /// Create config with default pattern-based classification enabled.
    pub fn with_default_patterns() -> Self {
        Self {
            categories: HashMap::new(),
            use_default_patterns: true,
        }
    }

    /// Add a type to a category.
    pub fn add(&mut self, type_name: impl Into<String>, category: EventCategory) {
        self.categories.insert(type_name.into(), category);
    }

    /// Add a type to a category (builder pattern).
    pub fn with(mut self, type_name: impl Into<String>, category: EventCategory) -> Self {
        self.add(type_name, category);
        self
    }

    /// Get category for an event type.
    pub fn get_category(&self, type_name: &str) -> EventCategory {
        // Try exact match first
        if let Some(cat) = self.categories.get(type_name) {
            return *cat;
        }

        // Try simple name (after last dot)
        let simple_name = type_name.rsplit('.').next().unwrap_or(type_name);
        if let Some(cat) = self.categories.get(simple_name) {
            return *cat;
        }

        // Fall back to default patterns if enabled
        if self.use_default_patterns {
            return Self::classify_by_pattern(simple_name);
        }

        EventCategory::Default
    }

    /// Classify event by name patterns (fallback).
    fn classify_by_pattern(type_name: &str) -> EventCategory {
        if type_name.ends_with("Created") || type_name.ends_with("Completed") {
            EventCategory::Success
        } else if type_name.ends_with("Cancelled")
            || type_name.ends_with("Failed")
            || type_name.ends_with("Rejected")
        {
            EventCategory::Failure
        } else if type_name.ends_with("Added")
            || type_name.ends_with("Updated")
            || type_name.ends_with("Applied")
        {
            EventCategory::Progress
        } else if type_name.ends_with("Started") || type_name.ends_with("Initiated") {
            EventCategory::Info
        } else {
            EventCategory::Default
        }
    }
}

/// Plain stdout output.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdoutOutput;

impl LogOutput for StdoutOutput {
    fn write_event(&self, event: &DecodedEvent) {
        println!();
        println!("{}", "─".repeat(60));
        println!("{}:{}:{:010}", event.domain, event.root_id, event.sequence);
        println!("{}", event.type_name);
        println!("{}", "─".repeat(60));

        for line in event.content.lines() {
            println!("  {line}");
        }
    }
}

/// File output.
pub struct FileOutput {
    writer: Mutex<BufWriter<File>>,
}

impl FileOutput {
    /// Create a new file output.
    pub fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.as_ref())?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
        })
    }
}

impl LogOutput for FileOutput {
    fn write_event(&self, event: &DecodedEvent) {
        let mut writer = self.writer.lock().unwrap();

        let _ = writeln!(writer);
        let _ = writeln!(writer, "{}", "─".repeat(60));
        let _ = writeln!(
            writer,
            "{}:{}:{:010}",
            event.domain, event.root_id, event.sequence
        );
        let _ = writeln!(writer, "{}", event.type_name);
        let _ = writeln!(writer, "{}", "─".repeat(60));

        for line in event.content.lines() {
            let _ = writeln!(writer, "  {line}");
        }

        let _ = writer.flush();
    }
}

/// Colorized stdout output with configurable event categories.
///
/// Outputs directly to stdout with ANSI color codes based on event type.
/// For plain output, use [`StdoutOutput`] instead.
pub struct ColorizingOutput {
    config: EventColorConfig,
}

impl ColorizingOutput {
    /// Create a new colorizing output with custom config.
    pub fn new(config: EventColorConfig) -> Self {
        Self { config }
    }

    /// Create with default pattern-based classification.
    pub fn with_default_patterns() -> Self {
        Self {
            config: EventColorConfig::with_default_patterns(),
        }
    }
}

impl LogOutput for ColorizingOutput {
    fn write_event(&self, event: &DecodedEvent) {
        let category = self.config.get_category(event.type_name);
        let color = category.color();

        println!();
        println!("{BOLD}{}{RESET}", "─".repeat(60));
        println!(
            "{DIM}{}:{}:{:010}{RESET}",
            event.domain, event.root_id, event.sequence
        );
        println!("{BOLD}{color}{}{RESET}", event.type_name);
        println!("{}", "─".repeat(60));

        for line in event.content.lines() {
            println!("  {line}");
        }
    }
}

/// Type alias for boxed LogOutput.
pub type BoxedLogOutput = Box<dyn LogOutput>;

impl LogOutput for BoxedLogOutput {
    fn write_event(&self, event: &DecodedEvent) {
        (**self).write_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_event_color_config_simple_name_match() {
        let config = EventColorConfig::new().with("PlayerRegistered", EventCategory::Success);

        // Should match simple name extracted from full name
        assert_eq!(
            config.get_category("examples.PlayerRegistered"),
            EventCategory::Success
        );
    }

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
}
