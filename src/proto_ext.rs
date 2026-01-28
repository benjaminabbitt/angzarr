//! Extension traits for proto types.
//!
//! Provides convenient accessor methods for common patterns like extracting
//! domain, correlation_id, and root_id from Cover-bearing types.

use crate::proto::{CommandBook, Cover, EventBook};

/// Extension trait for types with an optional Cover.
///
/// Provides convenient accessors for domain, correlation_id, and root_id
/// without verbose `.cover.as_ref().map(...)` chains.
pub trait CoverExt {
    /// Get the cover, if present.
    fn cover(&self) -> Option<&Cover>;

    /// Get the domain from the cover, or "unknown" if missing.
    fn domain(&self) -> &str {
        self.cover().map(|c| c.domain.as_str()).unwrap_or("unknown")
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

    /// Generate a cache key for this entity based on domain + root.
    ///
    /// Used for caching aggregate state during saga retry to avoid redundant fetches.
    fn cache_key(&self) -> String {
        let domain = self.domain();
        let root = self.root_id_hex().unwrap_or_default();
        format!("{domain}:{root}")
    }
}

impl Cover {
    /// Generate a cache key for this cover based on domain + root.
    pub fn cache_key(&self) -> String {
        let root = self
            .root
            .as_ref()
            .map(|u| hex::encode(&u.value))
            .unwrap_or_default();
        format!("{}:{}", self.domain, root)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::Uuid as ProtoUuid;

    fn make_cover(domain: &str, correlation_id: &str, root: Option<uuid::Uuid>) -> Cover {
        Cover {
            domain: domain.to_string(),
            correlation_id: correlation_id.to_string(),
            root: root.map(|u| ProtoUuid {
                value: u.as_bytes().to_vec(),
            }),
        }
    }

    #[test]
    fn test_event_book_with_cover() {
        let root = uuid::Uuid::new_v4();
        let book = EventBook {
            cover: Some(make_cover("orders", "corr-123", Some(root))),
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        };

        assert_eq!(book.domain(), "orders");
        assert_eq!(book.correlation_id(), "corr-123");
        assert!(book.has_correlation_id());
        assert_eq!(book.root_uuid(), Some(root));
        assert_eq!(book.root_id_hex(), Some(hex::encode(root.as_bytes())));
    }

    #[test]
    fn test_event_book_without_cover() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            snapshot_state: None,
        };

        assert_eq!(book.domain(), "unknown");
        assert_eq!(book.correlation_id(), "");
        assert!(!book.has_correlation_id());
        assert_eq!(book.root_uuid(), None);
        assert_eq!(book.root_id_hex(), None);
    }

    #[test]
    fn test_command_book_with_cover() {
        let book = CommandBook {
            cover: Some(make_cover("inventory", "corr-456", None)),
            pages: vec![],
            saga_origin: None,
        };

        assert_eq!(book.domain(), "inventory");
        assert_eq!(book.correlation_id(), "corr-456");
        assert!(book.has_correlation_id());
        assert_eq!(book.root_uuid(), None);
    }
}
