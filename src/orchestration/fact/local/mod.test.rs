use std::sync::Arc;

use tokio::sync::Mutex;

use super::*;

/// Mock router that tracks calls and can be configured to succeed or fail.
struct MockRouter {
    calls: Mutex<Vec<EventBook>>,
    should_fail: bool,
}

impl MockRouter {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            should_fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            should_fail: true,
        }
    }
}

#[async_trait]
impl FactRouterExecutor for MockRouter {
    async fn execute_fact(
        &self,
        fact: EventBook,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.calls.lock().await.push(fact);
        if self.should_fail {
            Err("mock failure".into())
        } else {
            Ok(())
        }
    }
}

/// LocalFactExecutor delegates to router on success.
#[tokio::test]
async fn test_inject_delegates_to_router() {
    let router = Arc::new(MockRouter::new());
    let executor = LocalFactExecutor::new(router.clone());

    let fact = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };

    let result = executor.inject(fact).await;
    assert!(result.is_ok());
    assert_eq!(router.calls.lock().await.len(), 1);
}

/// LocalFactExecutor maps router error to FactInjectionError::Internal.
#[tokio::test]
async fn test_inject_maps_router_error() {
    let router = Arc::new(MockRouter::failing());
    let executor = LocalFactExecutor::new(router);

    let fact = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "orders".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };

    let result = executor.inject(fact).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FactInjectionError::Internal(msg) => {
            assert!(msg.contains("mock failure"), "got: {msg}");
        }
        other => panic!("Expected Internal, got {:?}", other),
    }
}
