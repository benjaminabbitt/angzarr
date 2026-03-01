//! Chained DLQ publisher for priority list fallback.
//!
//! Tries each target in order until one succeeds.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::warn;

use super::error::DlqError;
use super::{AngzarrDeadLetter, DeadLetterPublisher};

/// Chained DLQ publisher that tries targets in priority order.
///
/// Each target is tried in sequence until one succeeds.
/// If all targets fail, returns the last error.
pub struct ChainedDlqPublisher {
    targets: Vec<Arc<dyn DeadLetterPublisher>>,
}

impl ChainedDlqPublisher {
    /// Create a new chained publisher with the given targets.
    pub fn new(targets: Vec<Arc<dyn DeadLetterPublisher>>) -> Self {
        Self { targets }
    }
}

#[async_trait]
impl DeadLetterPublisher for ChainedDlqPublisher {
    async fn publish(&self, dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
        let mut last_error = None;

        for (i, target) in self.targets.iter().enumerate() {
            match target.publish(dead_letter.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    warn!(
                        target_index = i,
                        error = %e,
                        "DLQ target failed, trying next"
                    );
                    last_error = Some(e);
                }
            }
        }

        // All targets failed
        Err(last_error.unwrap_or(DlqError::NotConfigured))
    }

    fn is_configured(&self) -> bool {
        !self.targets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dlq::publishers::NoopDeadLetterPublisher;
    use crate::dlq::DeadLetterPayload;
    use crate::proto::EventBook;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FailingPublisher {
        call_count: AtomicUsize,
    }

    impl FailingPublisher {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl DeadLetterPublisher for FailingPublisher {
        async fn publish(&self, _dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Err(DlqError::PublishFailed("intentional failure".to_string()))
        }
    }

    struct SuccessPublisher {
        call_count: AtomicUsize,
    }

    impl SuccessPublisher {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl DeadLetterPublisher for SuccessPublisher {
        async fn publish(&self, _dead_letter: AngzarrDeadLetter) -> Result<(), DlqError> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn make_test_dead_letter() -> AngzarrDeadLetter {
        AngzarrDeadLetter {
            cover: None,
            payload: DeadLetterPayload::Events(EventBook::default()),
            rejection_reason: "test".to_string(),
            rejection_details: None,
            occurred_at: None,
            metadata: HashMap::new(),
            source_component: "test".to_string(),
            source_component_type: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_chained_first_succeeds() {
        let success = Arc::new(SuccessPublisher::new());
        let failing = Arc::new(FailingPublisher::new());

        let chained = ChainedDlqPublisher::new(vec![
            success.clone() as Arc<dyn DeadLetterPublisher>,
            failing.clone() as Arc<dyn DeadLetterPublisher>,
        ]);

        let result = chained.publish(make_test_dead_letter()).await;
        assert!(result.is_ok());
        assert_eq!(success.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(failing.call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_chained_fallback_to_second() {
        let failing = Arc::new(FailingPublisher::new());
        let success = Arc::new(SuccessPublisher::new());

        let chained = ChainedDlqPublisher::new(vec![
            failing.clone() as Arc<dyn DeadLetterPublisher>,
            success.clone() as Arc<dyn DeadLetterPublisher>,
        ]);

        let result = chained.publish(make_test_dead_letter()).await;
        assert!(result.is_ok());
        assert_eq!(failing.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(success.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_chained_all_fail() {
        let failing1 = Arc::new(FailingPublisher::new());
        let failing2 = Arc::new(FailingPublisher::new());

        let chained = ChainedDlqPublisher::new(vec![
            failing1.clone() as Arc<dyn DeadLetterPublisher>,
            failing2.clone() as Arc<dyn DeadLetterPublisher>,
        ]);

        let result = chained.publish(make_test_dead_letter()).await;
        assert!(result.is_err());
        assert_eq!(failing1.call_count.load(Ordering::SeqCst), 1);
        assert_eq!(failing2.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_chained_empty_not_configured() {
        let chained = ChainedDlqPublisher::new(vec![]);
        assert!(!chained.is_configured());

        let result = chained.publish(make_test_dead_letter()).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_chained_is_configured() {
        let chained = ChainedDlqPublisher::new(vec![Arc::new(NoopDeadLetterPublisher)]);
        assert!(chained.is_configured());
    }
}
