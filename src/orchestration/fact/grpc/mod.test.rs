use std::collections::HashMap;

use super::*;

/// GrpcFactExecutor returns AggregateNotFound for unknown domain.
#[tokio::test]
async fn test_inject_unknown_domain_returns_not_found() {
    let executor = GrpcFactExecutor::new(HashMap::new());
    let fact = EventBook {
        cover: Some(crate::proto::Cover {
            domain: "unknown".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let result = executor.inject(fact).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FactInjectionError::AggregateNotFound { domain } => {
            assert_eq!(domain, "unknown");
        }
        other => panic!("Expected AggregateNotFound, got {:?}", other),
    }
}
