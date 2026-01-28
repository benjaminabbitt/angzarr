//! Validation helpers for command handler precondition checks.
//!
//! Eliminates repeated validation boilerplate across aggregate handlers.

use crate::{BusinessError, Result};

/// Require that an entity exists (identity field is non-empty).
pub fn require_exists(field: &str, error_msg: &str) -> Result<()> {
    if field.is_empty() {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

/// Require that an entity does NOT exist (identity field is empty).
pub fn require_not_exists(field: &str, error_msg: &str) -> Result<()> {
    if !field.is_empty() {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

/// Require that a numeric value is positive (> 0).
pub fn require_positive(value: i32, error_msg: &str) -> Result<()> {
    if value <= 0 {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

/// Require that a numeric value is non-negative (>= 0).
pub fn require_non_negative(value: i32, error_msg: &str) -> Result<()> {
    if value < 0 {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

/// Require that a slice is not empty.
pub fn require_not_empty<T>(items: &[T], error_msg: &str) -> Result<()> {
    if items.is_empty() {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

/// Require that the current status matches the expected value.
pub fn require_status(actual: &str, expected: &str, error_msg: &str) -> Result<()> {
    if actual != expected {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

/// Require that the current status does NOT match the forbidden value.
pub fn require_status_not(actual: &str, forbidden: &str, error_msg: &str) -> Result<()> {
    if actual == forbidden {
        return Err(BusinessError::Rejected(error_msg.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_exists_passes_when_non_empty() {
        assert!(require_exists("value", "error").is_ok());
    }

    #[test]
    fn test_require_exists_fails_when_empty() {
        let err = require_exists("", "entity not found").unwrap_err();
        assert!(err.to_string().contains("entity not found"));
    }

    #[test]
    fn test_require_not_exists_passes_when_empty() {
        assert!(require_not_exists("", "error").is_ok());
    }

    #[test]
    fn test_require_not_exists_fails_when_non_empty() {
        let err = require_not_exists("value", "already exists").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn test_require_positive_passes() {
        assert!(require_positive(1, "error").is_ok());
        assert!(require_positive(100, "error").is_ok());
    }

    #[test]
    fn test_require_positive_fails_on_zero() {
        let err = require_positive(0, "must be positive").unwrap_err();
        assert!(err.to_string().contains("must be positive"));
    }

    #[test]
    fn test_require_positive_fails_on_negative() {
        assert!(require_positive(-1, "error").is_err());
    }

    #[test]
    fn test_require_non_negative_passes() {
        assert!(require_non_negative(0, "error").is_ok());
        assert!(require_non_negative(1, "error").is_ok());
    }

    #[test]
    fn test_require_non_negative_fails() {
        assert!(require_non_negative(-1, "error").is_err());
    }

    #[test]
    fn test_require_not_empty_passes() {
        assert!(require_not_empty(&[1, 2, 3], "error").is_ok());
    }

    #[test]
    fn test_require_not_empty_fails() {
        let empty: &[i32] = &[];
        let err = require_not_empty(empty, "items required").unwrap_err();
        assert!(err.to_string().contains("items required"));
    }

    #[test]
    fn test_require_status_passes() {
        assert!(require_status("active", "active", "error").is_ok());
    }

    #[test]
    fn test_require_status_fails() {
        let err = require_status("pending", "active", "wrong status").unwrap_err();
        assert!(err.to_string().contains("wrong status"));
    }

    #[test]
    fn test_require_status_not_passes() {
        assert!(require_status_not("active", "checked_out", "error").is_ok());
    }

    #[test]
    fn test_require_status_not_fails() {
        let err =
            require_status_not("checked_out", "checked_out", "already checked out").unwrap_err();
        assert!(err.to_string().contains("already checked out"));
    }
}
