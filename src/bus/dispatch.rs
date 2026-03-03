//! Handler dispatch utilities.
//!
//! Provides common patterns for dispatching EventBooks to registered handlers.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::error;

#[cfg(test)]
use super::BusError;
use super::EventHandler;
use crate::proto::EventBook;

/// Dispatch an EventBook to all registered handlers.
///
/// Calls each handler in sequence, logging errors but continuing to subsequent
/// handlers. Returns `true` if all handlers succeeded, `false` if any failed.
///
/// # Example
///
/// ```ignore
/// let handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>> = ...;
/// let book: Arc<EventBook> = ...;
///
/// if dispatch_to_handlers(&handlers, &book).await {
///     message.ack().await;
/// } else {
///     message.nack().await;
/// }
/// ```
pub async fn dispatch_to_handlers(
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    book: &Arc<EventBook>,
) -> bool {
    let handlers_guard = handlers.read().await;
    let mut all_succeeded = true;

    for handler in handlers_guard.iter() {
        if let Err(e) = handler.handle(Arc::clone(book)).await {
            error!(error = %e, "Handler failed");
            all_succeeded = false;
        }
    }

    all_succeeded
}

/// Dispatch an EventBook to all handlers with domain context for logging.
///
/// Same as `dispatch_to_handlers` but includes domain in error messages
/// for better observability.
pub async fn dispatch_to_handlers_with_domain(
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
    book: &Arc<EventBook>,
    domain: &str,
) -> bool {
    let handlers_guard = handlers.read().await;
    let mut all_succeeded = true;

    for handler in handlers_guard.iter() {
        if let Err(e) = handler.handle(Arc::clone(book)).await {
            error!(domain = %domain, error = %e, "Handler failed");
            all_succeeded = false;
        }
    }

    all_succeeded
}

/// Result of processing a message through handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    /// All handlers succeeded, safe to acknowledge.
    Success,
    /// One or more handlers failed, consider retry.
    HandlerFailed,
    /// Message could not be decoded, no retry will help.
    DecodeError,
}

impl DispatchResult {
    /// Returns true if the message should be acknowledged (removed from queue).
    ///
    /// Decode errors should be acked to prevent infinite redelivery of bad messages.
    /// Success should obviously be acked.
    pub fn should_ack(&self) -> bool {
        matches!(self, Self::Success | Self::DecodeError)
    }

    /// Returns true if all handlers succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

/// Process a message payload through handlers.
///
/// Handles the complete decode → dispatch → result cycle:
/// 1. Decode EventBook from bytes
/// 2. Dispatch to all handlers
/// 3. Return appropriate result for ack decision
///
/// # Arguments
/// * `payload` - Raw bytes to decode as EventBook
/// * `handlers` - Registered event handlers
///
/// # Returns
/// * `DispatchResult::Success` - All handlers succeeded
/// * `DispatchResult::HandlerFailed` - At least one handler failed
/// * `DispatchResult::DecodeError` - Failed to decode payload
pub async fn process_message(
    payload: &[u8],
    handlers: &Arc<RwLock<Vec<Box<dyn EventHandler>>>>,
) -> DispatchResult {
    use prost::Message;

    match EventBook::decode(payload) {
        Ok(book) => {
            let book = Arc::new(book);
            if dispatch_to_handlers(handlers, &book).await {
                DispatchResult::Success
            } else {
                DispatchResult::HandlerFailed
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to decode EventBook");
            DispatchResult::DecodeError
        }
    }
}

#[cfg(test)]
#[path = "dispatch.test.rs"]
mod tests;
