//! Input validation for external data.
//!
//! Provides centralized validation for all fields that cross trust boundaries
//! (gRPC inputs, command fields, correlation IDs, etc.).

#![allow(clippy::result_large_err)]

use tonic::Status;

use crate::config::ResourceLimits;
use crate::proto::CommandBook;

/// Length limits for validated fields.
pub mod limits {
    /// Maximum domain name length (e.g., "order", "inventory").
    pub const MAX_DOMAIN_LENGTH: usize = 64;
    /// Maximum correlation ID length.
    pub const MAX_CORRELATION_ID_LENGTH: usize = 128;
    /// Maximum component name length (e.g., "saga-order-fulfillment").
    pub const MAX_COMPONENT_NAME_LENGTH: usize = 128;
    /// Maximum edition name length (e.g., "angzarr", "v2").
    pub const MAX_EDITION_LENGTH: usize = 64;
    /// Maximum type URL length (e.g., "type.googleapis.com/Order").
    pub const MAX_TYPE_URL_LENGTH: usize = 256;
}

/// Error constants for validation failures.
pub mod errmsg {
    pub const DOMAIN_EMPTY: &str = "domain name cannot be empty";
    pub const DOMAIN_TOO_LONG: &str = "domain name exceeds maximum length";
    pub const DOMAIN_INVALID_START: &str = "domain name must start with lowercase letter";
    pub const DOMAIN_INVALID_CHARS: &str =
        "domain name contains invalid characters (allowed: a-z, 0-9, _, -)";

    pub const CORRELATION_ID_TOO_LONG: &str = "correlation_id exceeds maximum length";
    pub const CORRELATION_ID_INVALID_CHARS: &str =
        "correlation_id contains invalid characters (allowed: a-zA-Z0-9_-)";

    pub const COMPONENT_NAME_EMPTY: &str = "component name cannot be empty";
    pub const COMPONENT_NAME_TOO_LONG: &str = "component name exceeds maximum length";
    pub const COMPONENT_NAME_INVALID_START: &str =
        "component name must start with lowercase letter";
    pub const COMPONENT_NAME_INVALID_CHARS: &str =
        "component name contains invalid characters (allowed: a-z, 0-9, _, -)";

    pub const EDITION_TOO_LONG: &str = "edition name exceeds maximum length";
    pub const EDITION_INVALID_START: &str = "edition name must start with lowercase letter";
    pub const EDITION_INVALID_CHARS: &str =
        "edition name contains invalid characters (allowed: a-z, 0-9, _, -)";

    pub const TOO_MANY_PAGES: &str = "command book exceeds maximum pages";
    pub const PAYLOAD_TOO_LARGE: &str = "page payload exceeds maximum size";
}

/// Validate a domain name.
///
/// Rules:
/// - Must not be empty
/// - Maximum 64 characters
/// - Must start with lowercase letter (a-z)
/// - May contain: lowercase letters (a-z), digits (0-9), underscore (_), hyphen (-)
///
/// Special domains like "_angzarr" are allowed (underscore prefix for internal use).
pub fn validate_domain(domain: &str) -> Result<(), Status> {
    if domain.is_empty() {
        return Err(Status::invalid_argument(errmsg::DOMAIN_EMPTY));
    }
    if domain.len() > limits::MAX_DOMAIN_LENGTH {
        return Err(Status::invalid_argument(format!(
            "{} (max: {}, got: {})",
            errmsg::DOMAIN_TOO_LONG,
            limits::MAX_DOMAIN_LENGTH,
            domain.len()
        )));
    }

    // Validate characters - first char has special rules
    let mut chars = domain.chars();
    // SAFETY: is_empty() check above guarantees at least one char
    let first_char = match chars.next() {
        Some(c) => c,
        None => return Err(Status::invalid_argument(errmsg::DOMAIN_EMPTY)),
    };

    // Allow underscore prefix for internal domains like "_angzarr"
    if !first_char.is_ascii_lowercase() && first_char != '_' {
        return Err(Status::invalid_argument(errmsg::DOMAIN_INVALID_START));
    }
    if !matches!(first_char, 'a'..='z' | '_') {
        return Err(Status::invalid_argument(errmsg::DOMAIN_INVALID_CHARS));
    }

    for ch in chars {
        if !matches!(ch, 'a'..='z' | '0'..='9' | '_' | '-') {
            return Err(Status::invalid_argument(errmsg::DOMAIN_INVALID_CHARS));
        }
    }

    Ok(())
}

/// Validate a correlation ID.
///
/// Rules:
/// - May be empty (correlation IDs are optional)
/// - Maximum 128 characters
/// - May contain: letters (a-zA-Z), digits (0-9), underscore (_), hyphen (-)
pub fn validate_correlation_id(id: &str) -> Result<(), Status> {
    if id.is_empty() {
        return Ok(());
    }
    if id.len() > limits::MAX_CORRELATION_ID_LENGTH {
        return Err(Status::invalid_argument(format!(
            "{} (max: {}, got: {})",
            errmsg::CORRELATION_ID_TOO_LONG,
            limits::MAX_CORRELATION_ID_LENGTH,
            id.len()
        )));
    }

    for ch in id.chars() {
        if !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-') {
            return Err(Status::invalid_argument(
                errmsg::CORRELATION_ID_INVALID_CHARS,
            ));
        }
    }

    Ok(())
}

/// Validate a component name.
///
/// Rules:
/// - Must not be empty
/// - Maximum 128 characters
/// - Must start with lowercase letter (a-z)
/// - May contain: lowercase letters (a-z), digits (0-9), underscore (_), hyphen (-)
pub fn validate_component_name(name: &str) -> Result<(), Status> {
    if name.is_empty() {
        return Err(Status::invalid_argument(errmsg::COMPONENT_NAME_EMPTY));
    }
    if name.len() > limits::MAX_COMPONENT_NAME_LENGTH {
        return Err(Status::invalid_argument(format!(
            "{} (max: {}, got: {})",
            errmsg::COMPONENT_NAME_TOO_LONG,
            limits::MAX_COMPONENT_NAME_LENGTH,
            name.len()
        )));
    }

    // Validate characters - first char has special rules
    let mut chars = name.chars();
    // SAFETY: is_empty() check above guarantees at least one char
    let first_char = match chars.next() {
        Some(c) => c,
        None => return Err(Status::invalid_argument(errmsg::COMPONENT_NAME_EMPTY)),
    };

    if !first_char.is_ascii_lowercase() {
        return Err(Status::invalid_argument(
            errmsg::COMPONENT_NAME_INVALID_START,
        ));
    }

    for ch in chars {
        if !matches!(ch, 'a'..='z' | '0'..='9' | '_' | '-') {
            return Err(Status::invalid_argument(
                errmsg::COMPONENT_NAME_INVALID_CHARS,
            ));
        }
    }

    Ok(())
}

/// Validate an edition name.
///
/// Rules:
/// - May be empty (defaults to "angzarr")
/// - Maximum 64 characters
/// - If non-empty, must start with lowercase letter (a-z)
/// - May contain: lowercase letters (a-z), digits (0-9), underscore (_), hyphen (-)
pub fn validate_edition(edition: &str) -> Result<(), Status> {
    if edition.is_empty() {
        return Ok(());
    }
    if edition.len() > limits::MAX_EDITION_LENGTH {
        return Err(Status::invalid_argument(format!(
            "{} (max: {}, got: {})",
            errmsg::EDITION_TOO_LONG,
            limits::MAX_EDITION_LENGTH,
            edition.len()
        )));
    }

    // Validate characters - first char has special rules
    let mut chars = edition.chars();
    // SAFETY: is_empty() check above guarantees at least one char
    let first_char = match chars.next() {
        Some(c) => c,
        None => return Ok(()), // Empty already handled above, but defensive
    };

    if !first_char.is_ascii_lowercase() {
        return Err(Status::invalid_argument(errmsg::EDITION_INVALID_START));
    }

    for ch in chars {
        if !matches!(ch, 'a'..='z' | '0'..='9' | '_' | '-') {
            return Err(Status::invalid_argument(errmsg::EDITION_INVALID_CHARS));
        }
    }

    Ok(())
}

/// Validate a command book against resource limits.
///
/// Checks:
/// - Number of pages does not exceed `max_pages_per_book`
/// - Each page payload does not exceed `max_payload_bytes`
pub fn validate_command_book(book: &CommandBook, limits: &ResourceLimits) -> Result<(), Status> {
    // Check pages count
    if book.pages.len() > limits.max_pages_per_book {
        return Err(Status::invalid_argument(format!(
            "{} (max: {}, got: {})",
            errmsg::TOO_MANY_PAGES,
            limits.max_pages_per_book,
            book.pages.len()
        )));
    }

    // Check each page payload size
    for (i, page) in book.pages.iter().enumerate() {
        if let Some(ref command) = page.command {
            if command.value.len() > limits.max_payload_bytes {
                return Err(Status::invalid_argument(format!(
                    "{} at page {} (max: {} bytes, got: {} bytes)",
                    errmsg::PAYLOAD_TOO_LARGE,
                    i,
                    limits.max_payload_bytes,
                    command.value.len()
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod domain_validation {
        use super::*;

        #[test]
        fn test_valid_domains() {
            assert!(validate_domain("order").is_ok());
            assert!(validate_domain("inventory").is_ok());
            assert!(validate_domain("order-fulfillment").is_ok());
            assert!(validate_domain("order_fulfillment").is_ok());
            assert!(validate_domain("order123").is_ok());
            assert!(validate_domain("a").is_ok());
            assert!(validate_domain("_angzarr").is_ok()); // internal domain
        }

        #[test]
        fn test_empty_domain() {
            let result = validate_domain("");
            assert!(result.is_err());
            assert!(result.unwrap_err().message().contains("empty"));
        }

        #[test]
        fn test_domain_too_long() {
            let long_domain = "a".repeat(65);
            let result = validate_domain(&long_domain);
            assert!(result.is_err());
            assert!(result.unwrap_err().message().contains("exceeds"));
        }

        #[test]
        fn test_domain_max_length() {
            let max_domain = "a".repeat(64);
            assert!(validate_domain(&max_domain).is_ok());
        }

        #[test]
        fn test_domain_invalid_start() {
            assert!(validate_domain("1order").is_err());
            assert!(validate_domain("-order").is_err());
            assert!(validate_domain("Order").is_err());
        }

        #[test]
        fn test_domain_invalid_chars() {
            assert!(validate_domain("order.fulfillment").is_err());
            assert!(validate_domain("order/fulfillment").is_err());
            assert!(validate_domain("order fulfillment").is_err());
            assert!(validate_domain("Order").is_err()); // uppercase
        }
    }

    mod correlation_id_validation {
        use super::*;

        #[test]
        fn test_valid_correlation_ids() {
            assert!(validate_correlation_id("").is_ok()); // empty is allowed
            assert!(validate_correlation_id("abc123").is_ok());
            assert!(validate_correlation_id("ABC123").is_ok());
            assert!(validate_correlation_id("order-123-abc").is_ok());
            assert!(validate_correlation_id("order_123_abc").is_ok());
            assert!(validate_correlation_id("OrderFulfillment123").is_ok());
        }

        #[test]
        fn test_correlation_id_too_long() {
            let long_id = "a".repeat(129);
            let result = validate_correlation_id(&long_id);
            assert!(result.is_err());
            assert!(result.unwrap_err().message().contains("exceeds"));
        }

        #[test]
        fn test_correlation_id_max_length() {
            let max_id = "a".repeat(128);
            assert!(validate_correlation_id(&max_id).is_ok());
        }

        #[test]
        fn test_correlation_id_invalid_chars() {
            assert!(validate_correlation_id("order.123").is_err());
            assert!(validate_correlation_id("order/123").is_err());
            assert!(validate_correlation_id("order 123").is_err());
            assert!(validate_correlation_id("order@123").is_err());
        }
    }

    mod component_name_validation {
        use super::*;

        #[test]
        fn test_valid_component_names() {
            assert!(validate_component_name("inventory").is_ok());
            assert!(validate_component_name("saga-order-fulfillment").is_ok());
            assert!(validate_component_name("projector-inventory-stock").is_ok());
            assert!(validate_component_name("agg123").is_ok());
        }

        #[test]
        fn test_empty_component_name() {
            let result = validate_component_name("");
            assert!(result.is_err());
            assert!(result.unwrap_err().message().contains("empty"));
        }

        #[test]
        fn test_component_name_too_long() {
            let long_name = "a".repeat(129);
            let result = validate_component_name(&long_name);
            assert!(result.is_err());
        }

        #[test]
        fn test_component_name_invalid_start() {
            assert!(validate_component_name("1saga").is_err());
            assert!(validate_component_name("-saga").is_err());
            assert!(validate_component_name("Saga").is_err());
            assert!(validate_component_name("_saga").is_err()); // unlike domain, no underscore prefix
        }

        #[test]
        fn test_component_name_invalid_chars() {
            assert!(validate_component_name("saga.order").is_err());
            assert!(validate_component_name("saga/order").is_err());
            assert!(validate_component_name("Saga-Order").is_err()); // uppercase
        }
    }

    mod edition_validation {
        use super::*;

        #[test]
        fn test_valid_editions() {
            assert!(validate_edition("").is_ok()); // empty is allowed (defaults to "angzarr")
            assert!(validate_edition("angzarr").is_ok());
            assert!(validate_edition("v2").is_ok());
            assert!(validate_edition("edition-123").is_ok());
            assert!(validate_edition("edition_123").is_ok());
        }

        #[test]
        fn test_edition_too_long() {
            let long_edition = "a".repeat(65);
            let result = validate_edition(&long_edition);
            assert!(result.is_err());
        }

        #[test]
        fn test_edition_max_length() {
            let max_edition = "a".repeat(64);
            assert!(validate_edition(&max_edition).is_ok());
        }

        #[test]
        fn test_edition_invalid_start() {
            assert!(validate_edition("1edition").is_err());
            assert!(validate_edition("-edition").is_err());
            assert!(validate_edition("Edition").is_err());
        }

        #[test]
        fn test_edition_invalid_chars() {
            assert!(validate_edition("edition.v2").is_err());
            assert!(validate_edition("Edition").is_err()); // uppercase
        }
    }

    mod resource_limits_validation {
        use super::*;
        use crate::proto::{CommandPage, Cover};
        use prost_types::Any;

        fn make_command_book(pages: Vec<CommandPage>) -> CommandBook {
            CommandBook {
                cover: Some(Cover {
                    domain: "test".to_string(),
                    root: None,
                    correlation_id: String::new(),
                    edition: None,
                }),
                pages,
                saga_origin: None,
            }
        }

        fn make_page_with_payload(size: usize) -> CommandPage {
            CommandPage {
                sequence: 0,
                command: Some(Any {
                    type_url: "test/Command".to_string(),
                    value: vec![0u8; size],
                }),
            }
        }

        #[test]
        fn test_empty_command_book() {
            let book = make_command_book(vec![]);
            let limits = ResourceLimits::default();
            assert!(validate_command_book(&book, &limits).is_ok());
        }

        #[test]
        fn test_command_book_within_limits() {
            let pages: Vec<_> = (0..10).map(|_| make_page_with_payload(1024)).collect();
            let book = make_command_book(pages);
            let limits = ResourceLimits::default();
            assert!(validate_command_book(&book, &limits).is_ok());
        }

        #[test]
        fn test_command_book_at_max_pages() {
            let pages: Vec<_> = (0..100).map(|_| make_page_with_payload(64)).collect();
            let book = make_command_book(pages);
            let limits = ResourceLimits::default();
            assert!(validate_command_book(&book, &limits).is_ok());
        }

        #[test]
        fn test_command_book_too_many_pages() {
            let pages: Vec<_> = (0..101).map(|_| make_page_with_payload(64)).collect();
            let book = make_command_book(pages);
            let limits = ResourceLimits::default();
            let result = validate_command_book(&book, &limits);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .message()
                .contains("exceeds maximum pages"));
        }

        #[test]
        fn test_command_book_at_max_payload() {
            let pages = vec![make_page_with_payload(256 * 1024)]; // exactly 256 KB
            let book = make_command_book(pages);
            let limits = ResourceLimits::default();
            assert!(validate_command_book(&book, &limits).is_ok());
        }

        #[test]
        fn test_command_book_payload_too_large() {
            let pages = vec![make_page_with_payload(256 * 1024 + 1)]; // 256 KB + 1
            let book = make_command_book(pages);
            let limits = ResourceLimits::default();
            let result = validate_command_book(&book, &limits);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .message()
                .contains("exceeds maximum size"));
        }

        #[test]
        fn test_command_book_with_custom_limits() {
            // Test with IPC limits (10 MB)
            let pages = vec![make_page_with_payload(5 * 1024 * 1024)]; // 5 MB
            let book = make_command_book(pages);
            let limits = ResourceLimits::for_ipc();
            assert!(validate_command_book(&book, &limits).is_ok());
        }
    }
}
