//! Unit tests for `persist_pm_event_book`, the PM persist + publish
//! seam extracted from `GrpcPMContext::persist_pm_events` so the
//! ephemeral mutants container (which runs `cargo test --lib` only)
//! can see the contract.
//!
//! The integration tests in `tests/pm_persist_event_store.rs` cover
//! the same surface end-to-end against a real SQLite store; the unit
//! tests here exist so mutation testing on the publish-step struct
//! fields (cover, pages, correlation_id stamping) actually gets a
//! chance to kill mutants — without `--lib`, `cargo mutants` cannot
//! observe an integration-only assertion.

use std::sync::Arc;

use prost_types::Any;
use uuid::Uuid;

use crate::bus::{EventBus, MockEventBus};
use crate::orchestration::command::CommandOutcome;
use crate::proto::{
    event_page, page_header, Cover, EventBook, EventPage, PageHeader, Uuid as ProtoUuid,
};
use crate::storage::{mock::MockEventStore, EventStore};

use super::persist_pm_event_book;

fn proto_uuid(u: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: u.as_bytes().to_vec(),
    }
}

fn pm_book(pm_domain: &str, pm_root: Uuid, correlation_id: &str, sequences: &[u32]) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: pm_domain.to_string(),
            root: Some(proto_uuid(pm_root)),
            correlation_id: correlation_id.to_string(),
            edition: None,
            ext: None,
        }),
        pages: sequences
            .iter()
            .map(|&seq| EventPage {
                header: Some(PageHeader {
                    sync_mode: None,
                    sequence_type: Some(page_header::SequenceType::Sequence(seq)),
                }),
                payload: Some(event_page::Payload::Event(Any {
                    type_url: "test.PmEvent".to_string(),
                    value: vec![],
                })),
                created_at: None,
                ..Default::default()
            })
            .collect(),
        snapshot: None,
        ..Default::default()
    }
}

/// R2-02-LIVE: the publish step ships the pages from
/// `process_events` directly. Pre-fix, the code re-read the store
/// after `event_store.add` and republished the full history.
#[tokio::test]
async fn persist_publishes_pages_from_process_events() {
    let store: Arc<dyn EventStore> = Arc::new(MockEventStore::new());
    let bus = Arc::new(MockEventBus::new());
    let bus_dyn: Arc<dyn EventBus> = bus.clone();
    let pm_root = Uuid::new_v4();

    let book = pm_book("pm", pm_root, "corr", &[0, 1, 2]);
    let outcome = persist_pm_event_book(&store, &bus_dyn, "pm", &book, "corr").await;
    assert!(matches!(outcome, CommandOutcome::Success(_)));

    let published = bus.take_published().await;
    assert_eq!(published.len(), 1, "expected one publish per persist call");
    assert_eq!(
        published[0].pages.len(),
        3,
        "publish must carry exactly the 3 handler-emitted pages"
    );
}

/// R2-02-LIVE: the publish step preserves the cover from
/// `process_events`. Mutating away the `cover` field would send a
/// book with no cover to the bus, breaking routing-key resolution.
#[tokio::test]
async fn persist_publishes_book_with_cover_present() {
    let store: Arc<dyn EventStore> = Arc::new(MockEventStore::new());
    let bus = Arc::new(MockEventBus::new());
    let bus_dyn: Arc<dyn EventBus> = bus.clone();
    let pm_root = Uuid::new_v4();

    let book = pm_book("pm-domain", pm_root, "corr", &[0]);
    persist_pm_event_book(&store, &bus_dyn, "pm-domain", &book, "corr").await;

    let published = bus.take_published().await;
    let cover = published[0]
        .cover
        .as_ref()
        .expect("published book must carry a cover");
    assert_eq!(
        cover.domain, "pm-domain",
        "cover.domain must come from process_events (not Default::default)"
    );
}

/// R2-02-LIVE: the publish step ALWAYS stamps the in-flight
/// `correlation_id` onto the published cover, even when the PM
/// service returned a book with a blank or stale correlation. This
/// is the coordinator's guarantee that downstream subscribers see
/// the same correlation across the cross-domain flow.
#[tokio::test]
async fn persist_stamps_in_flight_correlation_id_on_cover() {
    let store: Arc<dyn EventStore> = Arc::new(MockEventStore::new());
    let bus = Arc::new(MockEventBus::new());
    let bus_dyn: Arc<dyn EventBus> = bus.clone();
    let pm_root = Uuid::new_v4();

    // PM service returned a cover with NO correlation_id.
    let book = pm_book("pm", pm_root, "", &[0]);
    persist_pm_event_book(&store, &bus_dyn, "pm", &book, "in-flight").await;

    let published = bus.take_published().await;
    assert_eq!(
        published[0].cover.as_ref().unwrap().correlation_id,
        "in-flight",
        "publish cover must carry the in-flight correlation_id"
    );
}

/// Sanity contract: `event_store.add` failure surfaces as
/// `Rejected{Internal}` and the bus sees no publish.
#[tokio::test]
async fn persist_returns_rejected_internal_when_store_add_fails() {
    let mock_store = Arc::new(MockEventStore::new());
    mock_store.set_fail_on_add(true).await;
    let store: Arc<dyn EventStore> = mock_store.clone();
    let bus = Arc::new(MockEventBus::new());
    let bus_dyn: Arc<dyn EventBus> = bus.clone();
    let pm_root = Uuid::new_v4();

    let book = pm_book("pm", pm_root, "corr", &[0]);
    let outcome = persist_pm_event_book(&store, &bus_dyn, "pm", &book, "corr").await;
    match outcome {
        CommandOutcome::Rejected { code, .. } => assert_eq!(code, tonic::Code::Internal),
        other => panic!("expected Rejected, got {other:?}"),
    }
    assert_eq!(
        bus.take_published().await.len(),
        0,
        "no publish must occur when storage rejects the add"
    );
}
