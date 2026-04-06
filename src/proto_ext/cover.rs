//! Cover extension trait and implementations.
//!
//! Provides convenient accessors for domain, correlation_id, and root_id
//! from Cover-bearing types.

use crate::proto::{CommandBook, Cover, Edition, EventBook, Query};

use super::constants::{DEFAULT_EDITION, UNKNOWN_DOMAIN};

/// Extension trait for types with an optional Cover.
///
/// Provides convenient accessors for domain, correlation_id, and root_id
/// without verbose `.cover.as_ref().map(...)` chains.
pub trait CoverExt {
    /// Get the cover, if present.
    fn cover(&self) -> Option<&Cover>;

    /// Get the domain from the cover, or [`UNKNOWN_DOMAIN`] if missing.
    fn domain(&self) -> &str {
        self.cover()
            .map(|c| c.domain.as_str())
            .unwrap_or(UNKNOWN_DOMAIN)
    }

    /// Get the correlation_id from the cover, or empty string if missing.
    fn correlation_id(&self) -> &str {
        self.cover()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("")
    }

    /// Get the root UUID as a hex-encoded string, if present.
    fn root_id_hex(&self) -> Option<String> {
        self.cover()
            .and_then(|c| c.root.as_ref())
            .map(|u| hex::encode(&u.value))
    }

    /// Get the root UUID, if present.
    fn root_uuid(&self) -> Option<uuid::Uuid> {
        self.cover()
            .and_then(|c| c.root.as_ref())
            .and_then(|u| uuid::Uuid::from_slice(&u.value).ok())
    }

    /// Check if correlation_id is present and non-empty.
    fn has_correlation_id(&self) -> bool {
        !self.correlation_id().is_empty()
    }

    /// Get the edition name from the cover.
    ///
    /// Returns the explicit edition name if set and non-empty, otherwise
    /// defaults to the canonical timeline name (`"angzarr"`).
    fn edition(&self) -> &str {
        self.cover()
            .and_then(|c| c.edition.as_ref())
            .map(|e| e.name.as_str())
            .filter(|e| !e.is_empty())
            .unwrap_or(DEFAULT_EDITION)
    }

    /// Get the Edition struct from the cover, if present.
    fn edition_struct(&self) -> Option<&crate::proto::Edition> {
        self.cover().and_then(|c| c.edition.as_ref())
    }

    /// Get the edition name as an Option, without defaulting.
    ///
    /// Returns `Some(&str)` if edition is set and non-empty, `None` otherwise.
    fn edition_opt(&self) -> Option<&str> {
        self.cover()
            .and_then(|c| c.edition.as_ref())
            .map(|e| e.name.as_str())
            .filter(|n| !n.is_empty())
    }

    /// Compute the bus routing key: `"{domain}"`.
    ///
    /// The routing key is a transport concern used for bus subscription matching.
    /// Edition filtering is handled at the handler level, not the bus level.
    fn routing_key(&self) -> String {
        self.domain().to_string()
    }

    /// Generate a cache key for this entity based on edition + domain + root.
    ///
    /// Used for caching aggregate state during saga retry to avoid redundant fetches.
    /// Includes edition to prevent collision between aggregates in different timelines.
    fn cache_key(&self) -> String {
        let edition = self.edition();
        let domain = self.domain();
        let root = self.root_id_hex().unwrap_or_default();
        format!("{edition}:{domain}:{root}")
    }
}

impl CoverExt for EventBook {
    fn cover(&self) -> Option<&Cover> {
        self.cover.as_ref()
    }
}

impl CoverExt for CommandBook {
    fn cover(&self) -> Option<&Cover> {
        self.cover.as_ref()
    }
}

impl CoverExt for Query {
    fn cover(&self) -> Option<&Cover> {
        self.cover.as_ref()
    }
}

impl CoverExt for Cover {
    fn cover(&self) -> Option<&Cover> {
        Some(self)
    }
}

impl Cover {
    /// Stamp edition onto this cover if not already set.
    ///
    /// Sets the edition to the given name with no divergences if the cover's
    /// edition is None or has an empty name. Used by sagas and process managers
    /// to propagate source edition to outgoing covers and commands.
    pub fn stamp_edition_if_empty(&mut self, edition: &str) {
        if self.edition.as_ref().is_none_or(|e| e.name.is_empty()) {
            self.edition = Some(Edition {
                name: edition.to_string(),
                divergences: vec![],
            });
        }
    }
}
