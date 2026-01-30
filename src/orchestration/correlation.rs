//! Shared correlation ID generation.
//!
//! Uses UUIDv5 with the angzarr.dev namespace to produce deterministic
//! correlation IDs from command content when none is provided.

#![allow(clippy::result_large_err)]

use tonic::Status;

use crate::proto::CommandBook;

/// Returns existing correlation ID from the command's cover, or generates
/// a deterministic one from the serialized command content.
pub fn ensure_correlation_id(command_book: &CommandBook) -> Result<String, Status> {
    let existing = command_book
        .cover
        .as_ref()
        .map(|c| c.correlation_id.as_str())
        .unwrap_or("");

    if !existing.is_empty() {
        return Ok(existing.to_string());
    }

    use prost::Message;
    let mut buf = Vec::new();
    command_book.encode(&mut buf).map_err(|e| {
        Status::internal(format!("Failed to encode command for correlation ID: {e}"))
    })?;

    let angzarr_ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"angzarr.dev");
    Ok(uuid::Uuid::new_v5(&angzarr_ns, &buf).to_string())
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
    fn test_generates_valid_uuid() {
        let command = make_command_book(false);
        let result = ensure_correlation_id(&command).unwrap();

        assert!(!result.is_empty());
        assert!(uuid::Uuid::parse_str(&result).is_ok());
    }

    #[test]
    fn test_deterministic_generation() {
        let command1 = make_command_book(false);
        let command2 = command1.clone();

        let result1 = ensure_correlation_id(&command1).unwrap();
        let result2 = ensure_correlation_id(&command2).unwrap();

        assert_eq!(result1, result2);
    }
}
