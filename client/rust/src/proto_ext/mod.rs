//! Extension traits for proto types.
//!
//! Provides convenient accessor methods for common patterns like extracting
//! domain, correlation_id, and root_id from Cover-bearing types.
//!
//! ## Module Organization
//!
//! - [`constants`] - Shared constants (domain names, type URLs, headers)
//! - [`cover`] - CoverExt trait for accessing cover fields
//! - [`edition`] - EditionExt trait and Edition constructors
//! - [`uuid`] - UUID conversion traits
//! - [`pages`] - EventPageExt and CommandPageExt traits
//! - [`books`] - EventBookExt, CommandBookExt, and sequence helpers
//! - [`grpc`] - gRPC utilities for correlation and tracing

pub mod books;
pub mod constants;
pub mod cover;
pub mod edition;
pub mod grpc;
pub mod pages;
pub mod uuid;

// Re-export all public items for convenient imports
pub use books::{calculate_next_sequence, calculate_set_next_seq, CommandBookExt, EventBookExt};
pub use constants::{
    COMPONENT_REGISTERED_TYPE_URL, CORRELATION_ID_HEADER, DEFAULT_EDITION, META_ANGZARR_DOMAIN,
    PROJECTION_DOMAIN_PREFIX, PROJECTION_TYPE_URL, REGISTER_COMPONENT_TYPE_URL, UNKNOWN_DOMAIN,
    WILDCARD_DOMAIN,
};
pub use cover::CoverExt;
pub use edition::EditionExt;
pub use grpc::correlated_request;
pub use pages::{CommandPageExt, EventPageExt};
pub use uuid::{ProtoUuidExt, UuidExt};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandBook, Cover, Edition, EventBook, Uuid as ProtoUuid};

    fn make_cover(domain: &str, correlation_id: &str, root: Option<::uuid::Uuid>) -> Cover {
        Cover {
            domain: domain.to_string(),
            correlation_id: correlation_id.to_string(),
            root: root.map(|u| ProtoUuid {
                value: u.as_bytes().to_vec(),
            }),
            edition: None,
        }
    }

    #[test]
    fn test_event_book_with_cover() {
        let root = ::uuid::Uuid::new_v4();
        let book = EventBook {
            cover: Some(make_cover("orders", "corr-123", Some(root))),
            pages: vec![],
            snapshot: None,
            ..Default::default()
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
            ..Default::default()
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

    #[test]
    fn test_edition_main_timeline() {
        let edition = Edition::main_timeline();
        assert!(edition.is_main_timeline());
        assert_eq!(edition.name_or_default(), "angzarr");
    }

    #[test]
    fn test_edition_implicit() {
        let edition = Edition::implicit("v2");
        assert!(!edition.is_main_timeline());
        assert_eq!(edition.name, "v2");
        assert!(edition.divergences.is_empty());
    }

    #[test]
    fn test_edition_explicit_divergence() {
        let edition = Edition::explicit(
            "v2",
            vec![
                crate::proto::DomainDivergence {
                    domain: "order".to_string(),
                    sequence: 50,
                },
                crate::proto::DomainDivergence {
                    domain: "inventory".to_string(),
                    sequence: 75,
                },
            ],
        );
        assert_eq!(edition.divergence_for("order"), Some(50));
        assert_eq!(edition.divergence_for("inventory"), Some(75));
        assert_eq!(edition.divergence_for("other"), None);
    }

    #[test]
    fn test_edition_from_string() {
        let edition: Edition = "v2".into();
        assert_eq!(edition.name, "v2");
        assert!(edition.divergences.is_empty());
    }
}
