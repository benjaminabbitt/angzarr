//! Shared correlation ID extraction.
//!
//! Correlation IDs are client-provided for cross-domain workflows.
//! If not provided, the ID remains empty and process managers won't trigger.

#![allow(clippy::result_large_err)]

use std::sync::LazyLock;

use tonic::Status;

use crate::proto::CommandBook;
use crate::proto_ext::CoverExt;

/// Angzarr UUID namespace derived from DNS-based UUIDv5.
///
/// Used for deterministic UUID generation (e.g., component name to root UUID).
pub static ANGZARR_UUID_NAMESPACE: LazyLock<uuid::Uuid> =
    LazyLock::new(|| uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev"));

/// Extract existing correlation ID from command. Does not auto-generate.
///
/// Correlation ID is client-provided for cross-domain workflows.
/// If not provided, returns empty string (PMs won't trigger).
pub fn ensure_correlation_id(command_book: &CommandBook) -> Result<String, Status> {
    Ok(command_book.correlation_id().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandPage, Cover, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_command_book(with_correlation: bool) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: uuid::Uuid::new_v4().as_bytes().to_vec(),
                }),
                correlation_id: if with_correlation {
                    "test-correlation-id".to_string()
                } else {
                    String::new()
                },
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(Any {
                    type_url: "test.Command".to_string(),
                    value: vec![],
                }),
            }],
            saga_origin: None,
        }
    }

    #[test]
    fn test_existing_correlation_id_preserved() {
        let command = make_command_book(true);
        let result = ensure_correlation_id(&command).unwrap();
        assert_eq!(result, "test-correlation-id");
    }

    #[test]
    fn test_empty_correlation_id_stays_empty() {
        let command = make_command_book(false);
        let result = ensure_correlation_id(&command).unwrap();
        assert!(result.is_empty());
    }
}
