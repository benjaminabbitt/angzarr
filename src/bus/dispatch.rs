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
mod tests {
    //! Tests for event dispatch to registered handlers.
    //!
    //! Dispatch utilities route EventBooks to all registered handlers. The
    //! dispatch contract is critical for reliability:
    //!
    //! - All handlers are called, even if earlier handlers fail
    //! - Return value indicates whether all handlers succeeded
    //! - Decode errors are distinguishable from handler failures (affects ack)
    //!
    //! These behaviors ensure partial failures don't prevent delivery to
    //! healthy handlers while enabling correct retry/DLQ decisions.

    use super::*;
    use futures::future::BoxFuture;

    // ============================================================================
    // Test Doubles
    // ============================================================================

    /// Handler that always succeeds.
    struct SuccessHandler;
    impl EventHandler for SuccessHandler {
        fn handle(&self, _book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
            Box::pin(async { Ok(()) })
        }
    }

    /// Handler that always fails — simulates business logic errors.
    struct FailHandler;
    impl EventHandler for FailHandler {
        fn handle(&self, _book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
            Box::pin(async {
                Err(BusError::ProjectorFailed {
                    name: "test".to_string(),
                    message: "test failure".to_string(),
                })
            })
        }
    }

    // ============================================================================
    // Dispatch Tests
    // ============================================================================

    /// All handlers succeed → dispatch returns true.
    #[tokio::test]
    async fn test_dispatch_success() {
        let handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>> =
            Arc::new(RwLock::new(vec![Box::new(SuccessHandler)]));
        let book = Arc::new(EventBook::default());

        assert!(dispatch_to_handlers(&handlers, &book).await);
    }

    /// Handler failure → dispatch returns false.
    ///
    /// Caller uses return value to decide ack/nack. False means event
    /// should be retried or sent to DLQ.
    #[tokio::test]
    async fn test_dispatch_failure() {
        let handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>> =
            Arc::new(RwLock::new(vec![Box::new(FailHandler)]));
        let book = Arc::new(EventBook::default());

        assert!(!dispatch_to_handlers(&handlers, &book).await);
    }

    /// Mixed success/failure → all handlers called, returns false.
    ///
    /// One failing handler must not prevent other handlers from executing.
    /// Event may be partially processed — caller decides retry strategy.
    #[tokio::test]
    async fn test_dispatch_mixed() {
        let handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>> = Arc::new(RwLock::new(vec![
            Box::new(SuccessHandler),
            Box::new(FailHandler),
            Box::new(SuccessHandler), // Should still be called
        ]));
        let book = Arc::new(EventBook::default());

        // Returns false because one handler failed
        assert!(!dispatch_to_handlers(&handlers, &book).await);
    }

    // ============================================================================
    // DispatchResult Tests
    // ============================================================================

    /// Ack decision: success and decode errors should ack.
    ///
    /// Decode errors are not retryable — the message is malformed and will
    /// never succeed. Acking prevents infinite redelivery.
    #[tokio::test]
    async fn test_dispatch_result_should_ack() {
        assert!(DispatchResult::Success.should_ack());
        assert!(DispatchResult::DecodeError.should_ack());
        assert!(!DispatchResult::HandlerFailed.should_ack());
    }

    /// Invalid protobuf returns DecodeError — should ack to prevent redelivery.
    #[tokio::test]
    async fn test_process_message_decode_error() {
        let handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>> =
            Arc::new(RwLock::new(vec![Box::new(SuccessHandler)]));

        let result = process_message(b"not valid protobuf", &handlers).await;
        assert_eq!(result, DispatchResult::DecodeError);
    }

    /// Valid protobuf with successful handler returns Success.
    #[tokio::test]
    async fn test_process_message_success() {
        use prost::Message;

        let handlers: Arc<RwLock<Vec<Box<dyn EventHandler>>>> =
            Arc::new(RwLock::new(vec![Box::new(SuccessHandler)]));

        let book = EventBook::default();
        let payload = book.encode_to_vec();

        let result = process_message(&payload, &handlers).await;
        assert_eq!(result, DispatchResult::Success);
    }
}
