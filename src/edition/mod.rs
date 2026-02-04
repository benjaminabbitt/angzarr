//! Core types and logic for editions, not specific to standalone runtime.

/// Serialized divergence point type for sequence-based divergence.
pub const DIVERGENCE_TYPE_SEQUENCE: &str = "sequence";
/// Serialized divergence point type for timestamp-based divergence.
pub const DIVERGENCE_TYPE_TIMESTAMP: &str = "timestamp";

/// How to locate the divergence point for an edition.
#[derive(Debug, Clone)]
pub enum DivergencePoint {
    /// Diverge at a specific event sequence number (inclusive).
    AtSequence(u32),
    /// Diverge at a specific timestamp (RFC 3339, inclusive).
    AtTimestamp(String),
}

/// Metadata describing an edition.
///
/// [`DEFAULT_EDITION`]: crate::orchestration::aggregate::DEFAULT_EDITION
#[derive(Debug, Clone)]
pub struct EditionMetadata {
    /// Edition name (used as domain prefix: `{name}.{domain}`).
    pub name: String,
    /// Where the edition diverges from the main timeline.
    pub divergence: DivergencePoint,
    /// When the edition was created (RFC 3339).
    pub created_at: String,
    /// Human-readable description.
    pub description: String,
}
