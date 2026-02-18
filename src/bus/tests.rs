use super::*;
use crate::proto::{event_page::Sequence, Cover, EventPage, Target, Uuid as ProtoUuid};
use prost_types::Any;

fn make_event_book(domain: &str, event_types: &[&str]) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: uuid::Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: "test-correlation".to_string(),
            edition: None,
        }),
        pages: event_types
            .iter()
            .enumerate()
            .map(|(i, et)| EventPage {
                sequence: Some(Sequence::Num(i as u32)),
                created_at: None,
                event: Some(Any {
                    type_url: format!("type.googleapis.com/example.{}", et),
                    value: vec![],
                }),
                external_payload: None,
            })
            .collect(),
        snapshot: None,
        ..Default::default()
    }
}

#[test]
fn test_messaging_config_default() {
    let config = MessagingConfig::default();
    assert_eq!(config.messaging_type, MessagingType::Amqp);
    assert_eq!(config.amqp.url, "amqp://localhost:5672");
}

#[test]
fn test_target_matches_domain_only() {
    let book = make_event_book("order", &["OrderCreated"]);
    let target = Target {
        domain: "order".to_string(),
        types: vec![],
    };
    assert!(target_matches(&book, &target));
}

#[test]
fn test_target_matches_wrong_domain() {
    let book = make_event_book("order", &["OrderCreated"]);
    let target = Target {
        domain: "inventory".to_string(),
        types: vec![],
    };
    assert!(!target_matches(&book, &target));
}

#[test]
fn test_target_matches_specific_event_type() {
    let book = make_event_book("order", &["OrderCreated", "OrderShipped"]);
    let target = Target {
        domain: "order".to_string(),
        types: vec!["OrderCreated".to_string()],
    };
    assert!(target_matches(&book, &target));
}

#[test]
fn test_target_matches_event_type_not_present() {
    let book = make_event_book("order", &["OrderCreated"]);
    let target = Target {
        domain: "order".to_string(),
        types: vec!["OrderShipped".to_string()],
    };
    assert!(!target_matches(&book, &target));
}

#[test]
fn test_any_target_matches_first() {
    let book = make_event_book("order", &["OrderCreated"]);
    let targets = vec![
        Target {
            domain: "order".to_string(),
            types: vec!["OrderCreated".to_string()],
        },
        Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];
    assert!(any_target_matches(&book, &targets));
}

#[test]
fn test_any_target_matches_second() {
    let book = make_event_book("inventory", &["StockReserved"]);
    let targets = vec![
        Target {
            domain: "order".to_string(),
            types: vec![],
        },
        Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];
    assert!(any_target_matches(&book, &targets));
}

#[test]
fn test_any_target_matches_none() {
    let book = make_event_book("customer", &["CustomerCreated"]);
    let targets = vec![
        Target {
            domain: "order".to_string(),
            types: vec![],
        },
        Target {
            domain: "inventory".to_string(),
            types: vec![],
        },
    ];
    assert!(!any_target_matches(&book, &targets));
}

// ============================================================================
// wrap_with_offloading tests
// ============================================================================

mod offloading_wrapper {
    use super::*;
    use crate::payload_store::FilesystemPayloadStore;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_wrap_with_offloading_none_store_returns_original() {
        let mock_bus = Arc::new(MockEventBus::new());
        let bus: Arc<dyn EventBus> = mock_bus.clone();

        let wrapped = wrap_with_offloading::<FilesystemPayloadStore>(bus.clone(), None, None);

        // Should return the same bus (not wrapped)
        // We can verify by checking max_message_size behavior
        assert_eq!(wrapped.max_message_size(), bus.max_message_size());
    }

    #[tokio::test]
    async fn test_wrap_with_offloading_with_store() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let mock_bus = Arc::new(MockEventBus::new());
        let bus: Arc<dyn EventBus> = mock_bus.clone();

        let wrapped = wrap_with_offloading(bus, Some(store), Some(1024));

        // Should work - we can publish through the wrapped bus
        let book = make_event_book("test", &["TestEvent"]);
        let result = wrapped.publish(Arc::new(book)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wrap_with_offloading_uses_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(FilesystemPayloadStore::new(temp_dir.path()).await.unwrap());

        let mock_bus = Arc::new(MockEventBus::new());
        let bus: Arc<dyn EventBus> = mock_bus.clone();

        let threshold = 100;
        let wrapped = wrap_with_offloading(bus, Some(store), Some(threshold));

        // Create a small event book that won't be offloaded
        let small_book = make_event_book("test", &["SmallEvent"]);
        wrapped.publish(Arc::new(small_book)).await.unwrap();

        // Check the published book - should still have inline event
        let published = mock_bus.take_published().await;
        assert_eq!(published.len(), 1);
        assert!(published[0].pages[0].event.is_some());
        assert!(published[0].pages[0].external_payload.is_none());
    }
}
