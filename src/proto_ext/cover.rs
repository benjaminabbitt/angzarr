//! Cover extension trait and implementations.
//!
//! Provides convenient accessors for domain, correlation_id, and root_id
//! from Cover-bearing types.

use crate::proto::{CommandBook, Cover, EventBook, Query};

use super::constants::UNKNOWN_DOMAIN;

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

    /// Get the edition name from the cover, or None if not set.
    fn edition(&self) -> Option<&str> {
        self.cover()
            .and_then(|c| c.edition.as_ref())
            .map(|e| e.name.as_str())
            .filter(|e| !e.is_empty())
    }

    /// Get the Edition struct from the cover, if present.
    fn edition_struct(&self) -> Option<&crate::proto::Edition> {
        self.cover().and_then(|c| c.edition.as_ref())
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
        let edition = self.edition().unwrap_or_default();
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
    /// Copy the `source` cover's edition (full struct, including
    /// divergences) onto this cover, overwriting whatever was here.
    ///
    /// **Always-override semantics** — the coordinator guarantees
    /// timeline consistency on saga / PM cross-domain emissions;
    /// handlers cannot escape into a different edition by setting
    /// their own outgoing cover. See
    /// `coordinator-contract/edition_propagation.feature` (audit
    /// #86 / C-0140 / C-0145) for the contract.
    ///
    /// If `source` has no edition, this clears the outgoing edition
    /// — required by the contract: a main-timeline source produces
    /// a main-timeline outgoing cover regardless of what the handler
    /// set.
    pub fn propagate_edition_from(&mut self, source: &Cover) {
        self.edition = source.edition.clone();
    }
}
