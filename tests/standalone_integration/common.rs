//! Shared test fixtures and helpers for standalone integration tests.

pub use std::path::PathBuf;
pub use std::sync::atomic::{AtomicU32, Ordering};
pub use std::sync::Arc;

pub use async_trait::async_trait;
pub use prost_types::Any;
pub use tokio::sync::RwLock;
pub use tonic::Status;
pub use uuid::Uuid;

pub use angzarr::bus::ipc::{IpcBroker, IpcBrokerConfig, IpcConfig, IpcEventBus};
pub use angzarr::bus::{EventBus, EventHandler};
pub use angzarr::orchestration::aggregate::DEFAULT_EDITION;
pub use angzarr::proto::{
    event_page, CommandBook, CommandPage, ContextualCommand, Cover, EventBook, EventPage,
    Projection, SagaResponse, Uuid as ProtoUuid,
};
pub use angzarr::standalone::{
    AggregateHandler, ProjectionMode, ProjectorConfig, ProjectorHandler, RuntimeBuilder, SagaConfig,
    SagaHandler,
};

pub use std::os::unix::fs::FileTypeExt;
pub use std::time::Duration;

/// Simple test aggregate that echoes commands as events.
pub struct EchoAggregate {
    call_count: AtomicU32,
}

impl EchoAggregate {
    pub fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }

    pub fn calls(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AggregateHandler for EchoAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        // Get next sequence from prior events
        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        // Echo command as event
        let event_pages: Vec<EventPage> = command_book
            .pages
            .iter()
            .enumerate()
            .map(|(i, cmd_page)| EventPage {
                sequence: Some(event_page::Sequence::Num(next_seq + i as u32)),
                event: cmd_page.command.clone(),
                created_at: None,
            })
            .collect();

        Ok(EventBook {
            cover,
            pages: event_pages,
            snapshot: None,
        })
    }
}

/// Wrapper for Arc<EchoAggregate> to implement AggregateHandler.
pub struct EchoAggregateWrapper(pub Arc<EchoAggregate>);

#[async_trait]
impl AggregateHandler for EchoAggregateWrapper {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        self.0.handle(ctx).await
    }
}

/// Aggregate that produces N events per command.
pub struct MultiEventAggregate {
    events_per_command: u32,
}

impl MultiEventAggregate {
    pub fn new(events_per_command: u32) -> Self {
        Self { events_per_command }
    }
}

#[async_trait]
impl AggregateHandler for MultiEventAggregate {
    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command"))?;

        let cover = command_book.cover.clone();

        let next_seq = ctx
            .events
            .as_ref()
            .and_then(|e| e.pages.last())
            .and_then(|p| match &p.sequence {
                Some(event_page::Sequence::Num(n)) => Some(n + 1),
                _ => None,
            })
            .unwrap_or(0);

        let pages: Vec<EventPage> = (0..self.events_per_command)
            .map(|i| EventPage {
                sequence: Some(event_page::Sequence::Num(next_seq + i)),
                event: Some(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![i as u8],
                }),
                created_at: None,
            })
            .collect();

        Ok(EventBook {
            cover,
            pages,
            snapshot: None,
        })
    }
}

/// Shared state for recording events.
#[derive(Clone)]
pub struct RecordingHandlerState {
    events: Arc<RwLock<Vec<EventBook>>>,
}

impl RecordingHandlerState {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn received_count(&self) -> usize {
        self.events.read().await.len()
    }

    pub async fn get_events(&self) -> Vec<EventBook> {
        self.events.read().await.clone()
    }
}

/// Handler that records received events for verification.
pub struct RecordingHandler {
    state: RecordingHandlerState,
}

impl RecordingHandler {
    pub fn new(state: RecordingHandlerState) -> Self {
        Self { state }
    }
}

impl EventHandler for RecordingHandler {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> futures::future::BoxFuture<'static, Result<(), angzarr::bus::BusError>> {
        let events = self.state.events.clone();
        Box::pin(async move {
            events.write().await.push((*book).clone());
            Ok(())
        })
    }
}

pub fn create_test_command(domain: &str, root: Uuid, data: &[u8], sequence: u32) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: Uuid::new_v4().to_string(),
            edition: None,
        }),
        pages: vec![CommandPage {
            sequence,
            command: Some(Any {
                type_url: "test.TestCommand".to_string(),
                value: data.to_vec(),
            }),
        }],
        saga_origin: None,
    }
}

pub fn create_test_event_book(domain: &str, root: Uuid, sequence: u32) -> EventBook {
    EventBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: Uuid::new_v4().to_string(),
            edition: None,
        }),
        pages: vec![EventPage {
            sequence: Some(event_page::Sequence::Num(sequence)),
            event: Some(Any {
                type_url: "test.TestEvent".to_string(),
                value: vec![1, 2, 3],
            }),
            created_at: None,
        }],
        snapshot: None,
    }
}

pub fn temp_dir() -> PathBuf {
    let id = Uuid::new_v4().to_string()[..8].to_string();
    let path = PathBuf::from(format!("/tmp/angzarr-test-{}", id));
    std::fs::create_dir_all(&path).expect("Failed to create temp dir");
    path
}

pub fn cleanup_dir(path: &PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}
