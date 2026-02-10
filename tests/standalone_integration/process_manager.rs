//! Process Manager integration tests — state loading across invocations.

use crate::common::*;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use angzarr::proto::{event_page::Sequence, ComponentDescriptor, Target};
use angzarr::standalone::{ProcessManagerConfig, ProcessManagerHandler};

// ============================================================================
// Fixtures: PM that tracks state across invocations
// ============================================================================

/// PM that counts how many times it received prior state in handle().
struct StateTrackingPM {
    handle_count: AtomicU32,
    state_loaded_count: AtomicU32,
    state_was_loaded: AtomicBool,
}

impl StateTrackingPM {
    fn new() -> Self {
        Self {
            handle_count: AtomicU32::new(0),
            state_loaded_count: AtomicU32::new(0),
            state_was_loaded: AtomicBool::new(false),
        }
    }

    fn handle_count(&self) -> u32 {
        self.handle_count.load(Ordering::SeqCst)
    }

    fn state_loaded_count(&self) -> u32 {
        self.state_loaded_count.load(Ordering::SeqCst)
    }

    fn state_was_loaded(&self) -> bool {
        self.state_was_loaded.load(Ordering::SeqCst)
    }
}

impl ProcessManagerHandler for StateTrackingPM {
    fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: "state-tracking-pm".to_string(),
            component_type: "process_manager".to_string(),
            inputs: vec![Target {
                domain: "orders".to_string(),
                types: vec!["OrderPlaced".to_string()],
            }],
        }
    }

    fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
        vec![] // No additional destinations needed
    }

    fn handle(
        &self,
        trigger: &EventBook,
        process_state: Option<&EventBook>,
        _destinations: &[EventBook],
    ) -> (Vec<CommandBook>, Option<EventBook>) {
        self.handle_count.fetch_add(1, Ordering::SeqCst);

        // Track if we received prior state
        if let Some(state) = process_state {
            if !state.pages.is_empty() {
                self.state_loaded_count.fetch_add(1, Ordering::SeqCst);
                self.state_was_loaded.store(true, Ordering::SeqCst);
            }
        }

        let correlation_id = trigger
            .cover
            .as_ref()
            .map(|c| c.correlation_id.as_str())
            .unwrap_or_default();

        if correlation_id.is_empty() {
            return (vec![], None);
        }

        // Determine next sequence from existing state
        let next_seq = process_state
            .and_then(|s| s.pages.last())
            .and_then(|p| match &p.sequence {
                Some(Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        // Derive PM root from correlation_id
        let pm_root = Some(ProtoUuid {
            value: Uuid::new_v5(&Uuid::NAMESPACE_OID, correlation_id.as_bytes())
                .as_bytes()
                .to_vec(),
        });

        // Produce a PM event to track this invocation
        let pm_events = EventBook {
            cover: Some(Cover {
                domain: "state-tracking-pm".to_string(),
                root: pm_root,
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages: vec![EventPage {
                sequence: Some(Sequence::Num(next_seq)),
                created_at: None,
                event: Some(Any {
                    type_url: "pm.Invoked".to_string(),
                    value: vec![next_seq as u8],
                }),
            }],
            snapshot: None,
            ..Default::default()
        };

        (vec![], Some(pm_events))
    }
}

/// Wrapper to make Arc<StateTrackingPM> implement ProcessManagerHandler.
struct PMWrapper(Arc<StateTrackingPM>);

impl ProcessManagerHandler for PMWrapper {
    fn descriptor(&self) -> ComponentDescriptor {
        self.0.descriptor()
    }

    fn prepare(&self, trigger: &EventBook, process_state: Option<&EventBook>) -> Vec<Cover> {
        self.0.prepare(trigger, process_state)
    }

    fn handle(
        &self,
        trigger: &EventBook,
        process_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> (Vec<CommandBook>, Option<EventBook>) {
        self.0.handle(trigger, process_state, destinations)
    }
}

// ============================================================================
// Tests
// ============================================================================

/// Test that PM state is loaded on subsequent invocations with same correlation_id.
#[tokio::test]
async fn test_pm_state_loads_across_invocations() {
    let pm = Arc::new(StateTrackingPM::new());
    let pm_clone = pm.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_process_manager(
            "state-tracking-pm",
            PMWrapper(pm_clone),
            ProcessManagerConfig::new("state-tracking-pm"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();
    let correlation_id = "pm-state-test-123";

    // First trigger event
    let mut cmd1 = create_test_command("orders", Uuid::new_v4(), b"order-1", 0);
    if let Some(ref mut cover) = cmd1.cover {
        cover.correlation_id = correlation_id.to_string();
    }
    cmd1.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-1".to_vec(),
    });

    client.execute(cmd1).await.expect("First command failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // After first invocation, PM should have been called once, no prior state
    assert_eq!(pm.handle_count(), 1, "PM should be called once");
    assert_eq!(
        pm.state_loaded_count(),
        0,
        "First invocation should have no prior state"
    );

    // Second trigger event with SAME correlation_id
    let mut cmd2 = create_test_command("orders", Uuid::new_v4(), b"order-2", 0);
    if let Some(ref mut cover) = cmd2.cover {
        cover.correlation_id = correlation_id.to_string();
    }
    cmd2.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-2".to_vec(),
    });

    client.execute(cmd2).await.expect("Second command failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // After second invocation, PM should have received the state from first invocation
    assert_eq!(pm.handle_count(), 2, "PM should be called twice");
    assert!(
        pm.state_was_loaded(),
        "Second invocation should have loaded prior PM state"
    );
    assert_eq!(
        pm.state_loaded_count(),
        1,
        "Only second invocation should have prior state"
    );

    // Verify PM events are in storage
    let pm_store = runtime
        .event_store("state-tracking-pm")
        .expect("PM storage should exist");

    // The PM root is derived from correlation_id
    let pm_root = Uuid::new_v5(&Uuid::NAMESPACE_OID, correlation_id.as_bytes());
    let pm_events = pm_store
        .get("state-tracking-pm", DEFAULT_EDITION, pm_root)
        .await
        .expect("Should fetch PM events");

    assert_eq!(
        pm_events.len(),
        2,
        "PM should have 2 events (one per invocation)"
    );
}

/// Test that PM state is NOT shared across different correlation_ids.
#[tokio::test]
async fn test_pm_state_isolated_by_correlation_id() {
    let pm = Arc::new(StateTrackingPM::new());
    let pm_clone = pm.clone();

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        .register_aggregate("orders", EchoAggregate::new())
        .register_process_manager(
            "state-tracking-pm",
            PMWrapper(pm_clone),
            ProcessManagerConfig::new("state-tracking-pm"),
        )
        .build()
        .await
        .expect("Failed to build runtime");

    runtime.start().await.expect("Failed to start runtime");

    let client = runtime.command_client();

    // First trigger with correlation A
    let mut cmd1 = create_test_command("orders", Uuid::new_v4(), b"order-1", 0);
    if let Some(ref mut cover) = cmd1.cover {
        cover.correlation_id = "correlation-A".to_string();
    }
    cmd1.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-1".to_vec(),
    });

    client.execute(cmd1).await.expect("Command A failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Second trigger with DIFFERENT correlation B
    let mut cmd2 = create_test_command("orders", Uuid::new_v4(), b"order-2", 0);
    if let Some(ref mut cover) = cmd2.cover {
        cover.correlation_id = "correlation-B".to_string();
    }
    cmd2.pages[0].command = Some(Any {
        type_url: "orders.OrderPlaced".to_string(),
        value: b"order-2".to_vec(),
    });

    client.execute(cmd2).await.expect("Command B failed");
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Both invocations should have no prior state (different correlation_ids)
    assert_eq!(pm.handle_count(), 2, "PM should be called twice");
    assert_eq!(
        pm.state_loaded_count(),
        0,
        "Neither invocation should have prior state (different correlation_ids)"
    );
}
