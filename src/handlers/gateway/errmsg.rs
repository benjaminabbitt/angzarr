//! Error message constants for gateway handlers.
//!
//! User-facing error messages should be sanitized to avoid leaking infrastructure details.
//! Full error details are logged internally at DEBUG level.

/// Domain not registered in service discovery.
pub const DOMAIN_NOT_FOUND: &str = "Service not available for domain";

/// Component not found in service discovery.
pub const COMPONENT_NOT_FOUND: &str = "Service not available";

/// Service connection failed (sanitized - no address details).
pub const SERVICE_UNAVAILABLE: &str = "Service temporarily unavailable";

/// Internal infrastructure error (sanitized - no k8s details).
pub const INTERNAL_ERROR: &str = "Internal service error";

/// Event stream service unavailable.
pub const STREAM_UNAVAILABLE: &str = "Event streaming temporarily unavailable";

/// Projector not found for speculative execution.
pub const PROJECTOR_NOT_FOUND: &str = "Projector not available";
