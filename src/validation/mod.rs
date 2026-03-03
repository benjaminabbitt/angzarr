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
    let first_char = chars.next().expect("is_empty check guarantees char exists");

    // Allow underscore prefix for internal domains like "_angzarr"
    if !matches!(first_char, 'a'..='z' | '_') {
        return Err(Status::invalid_argument(errmsg::DOMAIN_INVALID_START));
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
    let first_char = chars.next().expect("is_empty check guarantees char exists");

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
    let first_char = chars.next().expect("is_empty check guarantees char exists");

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
        if let Some(crate::proto::command_page::Payload::Command(ref command)) = page.payload {
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
#[path = "mod.test.rs"]
mod tests;
