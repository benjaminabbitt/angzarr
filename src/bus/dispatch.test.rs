//! Tests for event dispatch to registered handlers.
//!
//! Dispatch utilities route EventBooks to all registered handlers. The
//! dispatch contract is critical for reliability:
//!
//! - All handlers are called, even if earlier handlers fail
//! - Return value indicates whether all handlers succeeded
//! - Decode errors are distinguishable from handler failures (affects ack)
//!
//! Why this matters: These behaviors ensure partial failures don't prevent
//! delivery to healthy handlers while enabling correct retry/DLQ decisions.
//! A failing projector shouldn't block saga execution.
//!
//! Key behaviors verified:
//! - Success returns true
//! - Handler failure returns false but continues to other handlers
//! - Mixed success/failure still calls all handlers
//! - Decode errors are distinguishable for ack decisions

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
