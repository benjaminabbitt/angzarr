//! Edition metadata types.
//!
//! **Merging divergent editions back to the main timeline is architecturally
//! infeasible and will never be supported.** Editions are one-way forks.

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
/// The edition name is used as the domain prefix: `{name}.{domain}`
/// (e.g., `"v2.order"`). The main timeline uses [`DEFAULT_EDITION`]
/// (`"angzarr"`) as its prefix.
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
