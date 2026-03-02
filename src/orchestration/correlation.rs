//! Shared correlation ID extraction.
//!
//! Correlation IDs are client-provided for cross-domain workflows.
//! If not provided, the ID remains empty and process managers won't trigger.

#![allow(clippy::result_large_err)]

use std::sync::LazyLock;

use tonic::Status;

use crate::proto::CommandBook;
use crate::proto_ext::CoverExt;
use crate::validation;

/// Angzarr UUID namespace derived from DNS-based UUIDv5.
///
/// Used for deterministic UUID generation (e.g., component name to root UUID).
pub static ANGZARR_UUID_NAMESPACE: LazyLock<uuid::Uuid> =
    LazyLock::new(|| uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev"));

/// Extract and validate correlation ID from command.
///
/// Correlation IDs are client-provided for cross-domain workflows.
/// Returns empty string if not provided—this is intentional:
/// - Process managers require correlation_id and will skip events without one
/// - This enables opt-in cross-domain tracking without polluting single-domain flows
///
/// Validates format if non-empty.
pub fn extract_correlation_id(command_book: &CommandBook) -> Result<String, Status> {
    let id = command_book.correlation_id().to_string();
    validation::validate_correlation_id(&id)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    //! Tests for correlation ID extraction from commands.
    //!
    //! Correlation IDs are the primary mechanism for cross-domain workflow tracking.
    //! They enable process managers to correlate events across multiple domains and
    //! provide end-to-end observability for saga flows. The extraction logic must:
    //! - Preserve client-provided correlation IDs exactly as given
    //! - Return empty string when not provided (allowing opt-in tracking)
    //! - Validate format when non-empty to prevent injection issues

    use super::*;
    use crate::proto::{command_page, CommandPage, Cover, MergeStrategy, Uuid as ProtoUuid};
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
                external_id: String::new(),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                payload: Some(command_page::Payload::Command(Any {
                    type_url: "test.Command".to_string(),
                    value: vec![],
                })),
                merge_strategy: MergeStrategy::MergeCommutative as i32,
            }],
            saga_origin: None,
        }
    }

    /// Client-provided correlation ID must be extracted verbatim.
    ///
    /// The framework must never modify or regenerate correlation IDs — they are
    /// client-controlled identifiers for cross-domain workflows. Altering them
    /// would break event correlation in process managers.
    #[test]
    fn test_extract_preserves_existing_id() {
        let command = make_command_book(true);
        let result = extract_correlation_id(&command).unwrap();
        assert_eq!(result, "test-correlation-id");
    }

    /// Missing correlation ID returns empty string, not an error.
    ///
    /// Cross-domain tracking is opt-in. Single-domain operations don't need
    /// correlation IDs, and generating them automatically would pollute PM
    /// state with unrelated events. Empty correlation_id signals "no PM routing".
    #[test]
    fn test_extract_returns_empty_when_not_provided() {
        let command = make_command_book(false);
        let result = extract_correlation_id(&command).unwrap();
        assert!(result.is_empty());
    }
}
