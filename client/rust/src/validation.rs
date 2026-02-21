//! Validation helpers for command handler precondition checks.
//!
//! Eliminates repeated validation boilerplate across aggregate handlers.
//!
//! # Example
//!
//! ```rust,ignore
//! use angzarr_client::validation::{require_exists, require_positive};
//!
//! fn handle_deposit(state: &PlayerState, amount: i64) -> CommandResult<EventBook> {
//!     require_exists(state.exists(), "Player does not exist")?;
//!     require_positive(amount, "amount")?;
//!     // ... rest of handler
//! }
//! ```

use crate::CommandRejectedError;

/// Require that an aggregate exists (has prior events).
pub fn require_exists(exists: bool, message: &str) -> Result<(), CommandRejectedError> {
    if !exists {
        return Err(CommandRejectedError::new(message));
    }
    Ok(())
}

/// Require that an aggregate does not exist.
pub fn require_not_exists(exists: bool, message: &str) -> Result<(), CommandRejectedError> {
    if exists {
        return Err(CommandRejectedError::new(message));
    }
    Ok(())
}

/// Require that a value is positive (greater than zero).
pub fn require_positive<T: PartialOrd + Default>(
    value: T,
    field_name: &str,
) -> Result<(), CommandRejectedError> {
    if value <= T::default() {
        return Err(CommandRejectedError::new(format!(
            "{} must be positive",
            field_name
        )));
    }
    Ok(())
}

/// Require that a value is non-negative (zero or greater).
pub fn require_non_negative<T: PartialOrd + Default>(
    value: T,
    field_name: &str,
) -> Result<(), CommandRejectedError> {
    if value < T::default() {
        return Err(CommandRejectedError::new(format!(
            "{} must be non-negative",
            field_name
        )));
    }
    Ok(())
}

/// Require that a string is not empty.
pub fn require_not_empty_str(value: &str, field_name: &str) -> Result<(), CommandRejectedError> {
    if value.is_empty() {
        return Err(CommandRejectedError::new(format!(
            "{} must not be empty",
            field_name
        )));
    }
    Ok(())
}

/// Require that a collection is not empty.
pub fn require_not_empty<T>(items: &[T], field_name: &str) -> Result<(), CommandRejectedError> {
    if items.is_empty() {
        return Err(CommandRejectedError::new(format!(
            "{} must not be empty",
            field_name
        )));
    }
    Ok(())
}

/// Require that a status matches an expected value.
pub fn require_status<T: PartialEq>(
    actual: T,
    expected: T,
    message: &str,
) -> Result<(), CommandRejectedError> {
    if actual != expected {
        return Err(CommandRejectedError::new(message));
    }
    Ok(())
}

/// Require that a status does not match a forbidden value.
pub fn require_status_not<T: PartialEq>(
    actual: T,
    forbidden: T,
    message: &str,
) -> Result<(), CommandRejectedError> {
    if actual == forbidden {
        return Err(CommandRejectedError::new(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_exists_passes() {
        assert!(require_exists(true, "should exist").is_ok());
    }

    #[test]
    fn test_require_exists_fails() {
        let result = require_exists(false, "Player does not exist");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "Player does not exist");
    }

    #[test]
    fn test_require_not_exists_passes() {
        assert!(require_not_exists(false, "should not exist").is_ok());
    }

    #[test]
    fn test_require_not_exists_fails() {
        let result = require_not_exists(true, "Player already exists");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "Player already exists");
    }

    #[test]
    fn test_require_positive_passes() {
        assert!(require_positive(1i64, "amount").is_ok());
        assert!(require_positive(100i32, "value").is_ok());
    }

    #[test]
    fn test_require_positive_fails_zero() {
        let result = require_positive(0i64, "amount");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "amount must be positive");
    }

    #[test]
    fn test_require_positive_fails_negative() {
        let result = require_positive(-5i64, "amount");
        assert!(result.is_err());
    }

    #[test]
    fn test_require_non_negative_passes() {
        assert!(require_non_negative(0i64, "balance").is_ok());
        assert!(require_non_negative(100i64, "balance").is_ok());
    }

    #[test]
    fn test_require_non_negative_fails() {
        let result = require_non_negative(-1i64, "balance");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "balance must be non-negative");
    }

    #[test]
    fn test_require_not_empty_str_passes() {
        assert!(require_not_empty_str("hello", "name").is_ok());
    }

    #[test]
    fn test_require_not_empty_str_fails() {
        let result = require_not_empty_str("", "name");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "name must not be empty");
    }

    #[test]
    fn test_require_not_empty_passes() {
        assert!(require_not_empty(&[1, 2, 3], "items").is_ok());
    }

    #[test]
    fn test_require_not_empty_fails() {
        let empty: Vec<i32> = vec![];
        let result = require_not_empty(&empty, "items");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "items must not be empty");
    }

    #[test]
    fn test_require_status_passes() {
        assert!(require_status("active", "active", "must be active").is_ok());
    }

    #[test]
    fn test_require_status_fails() {
        let result = require_status("pending", "active", "must be active");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "must be active");
    }

    #[test]
    fn test_require_status_not_passes() {
        assert!(require_status_not("active", "deleted", "cannot be deleted").is_ok());
    }

    #[test]
    fn test_require_status_not_fails() {
        let result = require_status_not("deleted", "deleted", "cannot be deleted");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().reason, "cannot be deleted");
    }
}
