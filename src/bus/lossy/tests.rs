use super::*;
use crate::bus::MockEventBus;
use crate::proto::{Cover, Uuid as ProtoUuid};
use uuid::Uuid;

fn make_event_book(domain: &str) -> Arc<EventBook> {
    Arc::new(EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: vec![],
        snapshot: None,
    })
}

#[test]
fn test_lossy_config_none() {
    let config = LossyConfig::none();
    assert_eq!(config.drop_rate, 0.0);
    assert!(!config.is_lossy());
}

#[test]
fn test_lossy_config_with_rate() {
    let config = LossyConfig::with_drop_rate(0.5);
    assert_eq!(config.drop_rate, 0.5);
    assert!(config.is_lossy());
}

#[test]
fn test_lossy_config_clamps_rate() {
    let low = LossyConfig::with_drop_rate(-0.5);
    assert_eq!(low.drop_rate, 0.0);

    let high = LossyConfig::with_drop_rate(1.5);
    assert_eq!(high.drop_rate, 1.0);
}

#[test]
fn test_lossy_config_drop_all() {
    let config = LossyConfig::drop_all();
    assert_eq!(config.drop_rate, 1.0);
    assert!(config.is_lossy());
}

#[tokio::test]
async fn test_passthrough_publishes_all() {
    let inner = MockEventBus::new();
    let lossy = LossyEventBus::passthrough(inner);

    for _ in 0..10 {
        lossy.publish(make_event_book("orders")).await.unwrap();
    }

    let (total, dropped, passed) = lossy.stats().snapshot();
    assert_eq!(total, 10);
    assert_eq!(dropped, 0);
    assert_eq!(passed, 10);
}

#[tokio::test]
async fn test_drop_all_drops_everything() {
    let inner = MockEventBus::new();
    let lossy = LossyEventBus::new(inner, LossyConfig::drop_all());

    for _ in 0..10 {
        lossy.publish(make_event_book("orders")).await.unwrap();
    }

    let (total, dropped, passed) = lossy.stats().snapshot();
    assert_eq!(total, 10);
    assert_eq!(dropped, 10);
    assert_eq!(passed, 0);
}

#[tokio::test]
async fn test_partial_drop_rate() {
    let inner = MockEventBus::new();
    let lossy = LossyEventBus::new(inner, LossyConfig::with_drop_rate(0.5).with_logging(false));

    // Publish many messages to get statistical significance
    for _ in 0..1000 {
        lossy.publish(make_event_book("orders")).await.unwrap();
    }

    let (total, dropped, passed) = lossy.stats().snapshot();
    assert_eq!(total, 1000);
    assert_eq!(dropped + passed, 1000);

    // With 1000 samples and 50% drop rate, we should be within 40-60%
    let observed_rate = lossy.stats().observed_drop_rate();
    assert!(
        observed_rate > 0.4 && observed_rate < 0.6,
        "Expected ~50% drop rate, got {:.2}%",
        observed_rate * 100.0
    );
}

#[tokio::test]
async fn test_stats_reset() {
    let inner = MockEventBus::new();
    let lossy = LossyEventBus::new(inner, LossyConfig::with_drop_rate(0.5).with_logging(false));

    for _ in 0..10 {
        lossy.publish(make_event_book("orders")).await.unwrap();
    }

    let (total, _, _) = lossy.stats().snapshot();
    assert_eq!(total, 10);

    lossy.stats().reset();

    let (total, dropped, passed) = lossy.stats().snapshot();
    assert_eq!(total, 0);
    assert_eq!(dropped, 0);
    assert_eq!(passed, 0);
}

#[tokio::test]
async fn test_inner_access() {
    let inner = MockEventBus::new();
    let mut lossy = LossyEventBus::passthrough(inner);

    // Access inner
    let _inner_ref = lossy.inner();
    let _inner_mut = lossy.inner_mut();

    // Consume and get inner back
    let _recovered = lossy.into_inner();
}

#[tokio::test]
async fn test_runtime_rate_change() {
    let inner = MockEventBus::new();
    let mut lossy = LossyEventBus::passthrough(inner);

    // Initially pass-through
    lossy.publish(make_event_book("orders")).await.unwrap();
    assert_eq!(lossy.stats().snapshot().2, 1); // passed = 1

    // Change to drop-all
    lossy.set_drop_rate(1.0);
    lossy.publish(make_event_book("orders")).await.unwrap();
    assert_eq!(lossy.stats().snapshot().1, 1); // dropped = 1
}
