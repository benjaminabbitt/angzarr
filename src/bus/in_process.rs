//! In-process event bus implementation.
//!
//! Routes events to in-process projectors and sagas without gRPC overhead.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::interfaces::event_bus::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::interfaces::projector::Projector;
use crate::interfaces::saga::Saga;
use crate::proto::{CommandBook, EventBook, Projection};

/// In-process event bus.
///
/// Routes events directly to registered projectors and sagas
/// without network overhead. Ideal for:
/// - Single-process applications
/// - Testing
/// - Embedded use cases
pub struct InProcessEventBus {
    projectors: RwLock<Vec<Arc<dyn Projector>>>,
    sagas: RwLock<Vec<Arc<dyn Saga>>>,
    /// Commands produced by sagas, to be processed by caller.
    pending_commands: RwLock<Vec<CommandBook>>,
}

impl InProcessEventBus {
    /// Create a new in-process event bus.
    pub fn new() -> Self {
        Self {
            projectors: RwLock::new(Vec::new()),
            sagas: RwLock::new(Vec::new()),
            pending_commands: RwLock::new(Vec::new()),
        }
    }

    /// Register an in-process projector.
    pub async fn add_projector(&self, projector: Box<dyn Projector>) {
        let projector: Arc<dyn Projector> = projector.into();
        info!(
            projector.name = %projector.name(),
            projector.domains = ?projector.domains(),
            "Registered in-process projector"
        );
        self.projectors.write().await.push(projector);
    }

    /// Register an in-process saga.
    pub async fn add_saga(&self, saga: Box<dyn Saga>) {
        let saga: Arc<dyn Saga> = saga.into();
        info!(
            saga.name = %saga.name(),
            saga.domains = ?saga.domains(),
            "Registered in-process saga"
        );
        self.sagas.write().await.push(saga);
    }

    /// Take any commands produced by sagas during publish.
    ///
    /// Call this after `publish()` to get commands that need processing.
    pub async fn take_pending_commands(&self) -> Vec<CommandBook> {
        std::mem::take(&mut *self.pending_commands.write().await)
    }

    /// Get the domain from an event book.
    fn get_domain(book: &EventBook) -> Option<&str> {
        book.cover.as_ref().map(|c| c.domain.as_str())
    }

    /// Check if a handler is interested in this domain.
    fn is_interested(handler_domains: &[String], event_domain: &str) -> bool {
        handler_domains.is_empty() || handler_domains.iter().any(|d| d == event_domain)
    }
}

impl Default for InProcessEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventBus for InProcessEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let domain = Self::get_domain(&book).unwrap_or("unknown");
        let mut projections: Vec<Projection> = Vec::new();

        // Collect projectors under read lock, then release before async calls
        let projector_list: Vec<_> = {
            let guard = self.projectors.read().await;
            guard
                .iter()
                .filter(|p| Self::is_interested(&p.domains(), domain))
                .cloned()
                .collect()
        };

        for projector in projector_list {
            match projector.project(&book).await {
                Ok(Some(projection)) => {
                    info!(
                        projector.name = %projector.name(),
                        domain = %domain,
                        "Projection produced"
                    );
                    if projector.is_synchronous() {
                        projections.push(projection);
                    }
                }
                Ok(None) => {
                    info!(
                        projector.name = %projector.name(),
                        domain = %domain,
                        "Projection completed"
                    );
                }
                Err(e) => {
                    if projector.is_synchronous() {
                        error!(
                            projector.name = %projector.name(),
                            error = %e,
                            "Synchronous projector failed"
                        );
                        return Err(BusError::ProjectorFailed {
                            name: projector.name().to_string(),
                            source: e,
                        });
                    }
                    warn!(
                        projector.name = %projector.name(),
                        error = %e,
                        "Async projector failed"
                    );
                }
            }
        }

        // Collect sagas under read lock, then release before async calls
        let sagas: Vec<_> = {
            let guard = self.sagas.read().await;
            guard
                .iter()
                .filter(|s| Self::is_interested(&s.domains(), domain))
                .cloned()
                .collect()
        };

        // Collect all commands first, then add to pending in one write
        let mut all_commands = Vec::new();

        for saga in sagas {
            match saga.handle(&book).await {
                Ok(commands) => {
                    if !commands.is_empty() {
                        info!(
                            saga.name = %saga.name(),
                            command_count = commands.len(),
                            "Saga produced commands"
                        );
                        all_commands.extend(commands);
                    }
                }
                Err(e) => {
                    if saga.is_synchronous() {
                        error!(
                            saga.name = %saga.name(),
                            error = %e,
                            "Synchronous saga failed"
                        );
                        return Err(BusError::SagaFailed {
                            name: saga.name().to_string(),
                            source: e,
                        });
                    }
                    warn!(
                        saga.name = %saga.name(),
                        error = %e,
                        "Async saga failed"
                    );
                }
            }
        }

        // Single write to pending_commands
        if !all_commands.is_empty() {
            self.pending_commands.write().await.extend(all_commands);
        }

        Ok(PublishResult { projections })
    }

    async fn subscribe(&self, _handler: Box<dyn EventHandler>) -> Result<()> {
        Err(BusError::SubscribeNotSupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::projector::{Projector, ProjectorError, Result as ProjectorResult};
    use crate::interfaces::saga::{Result as SagaResult, Saga, SagaError};
    use crate::proto::{event_page, CommandBook, CommandPage, Cover, EventPage, Projection};
    use crate::proto::Uuid as ProtoUuid;
    use prost_types::Any;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    fn make_event_book(domain: &str) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
                synchronous: false,
            }],
            snapshot: None,
        }
    }

    struct CountingProjector {
        name: String,
        domains: Vec<String>,
        count: AtomicUsize,
        synchronous: bool,
    }

    impl CountingProjector {
        fn new(name: &str, domains: Vec<String>) -> Self {
            Self {
                name: name.to_string(),
                domains,
                count: AtomicUsize::new(0),
                synchronous: false,
            }
        }

        fn call_count(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Projector for CountingProjector {
        fn name(&self) -> &str {
            &self.name
        }

        fn domains(&self) -> Vec<String> {
            self.domains.clone()
        }

        async fn project(&self, _book: &Arc<EventBook>) -> ProjectorResult<Option<Projection>> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }

        fn is_synchronous(&self) -> bool {
            self.synchronous
        }
    }

    struct FailingProjector {
        synchronous: bool,
    }

    #[async_trait]
    impl Projector for FailingProjector {
        fn name(&self) -> &str {
            "failing"
        }

        fn domains(&self) -> Vec<String> {
            vec![]
        }

        async fn project(&self, _book: &Arc<EventBook>) -> ProjectorResult<Option<Projection>> {
            Err(ProjectorError::Failed("intentional failure".to_string()))
        }

        fn is_synchronous(&self) -> bool {
            self.synchronous
        }
    }

    struct CommandProducingSaga {
        name: String,
        domains: Vec<String>,
        commands_to_produce: Mutex<Vec<CommandBook>>,
    }

    impl CommandProducingSaga {
        fn new(name: &str, domains: Vec<String>, commands: Vec<CommandBook>) -> Self {
            Self {
                name: name.to_string(),
                domains,
                commands_to_produce: Mutex::new(commands),
            }
        }
    }

    #[async_trait]
    impl Saga for CommandProducingSaga {
        fn name(&self) -> &str {
            &self.name
        }

        fn domains(&self) -> Vec<String> {
            self.domains.clone()
        }

        async fn handle(&self, _book: &Arc<EventBook>) -> SagaResult<Vec<CommandBook>> {
            let commands = std::mem::take(&mut *self.commands_to_produce.lock().await);
            Ok(commands)
        }
    }

    struct FailingSaga {
        synchronous: bool,
    }

    #[async_trait]
    impl Saga for FailingSaga {
        fn name(&self) -> &str {
            "failing_saga"
        }

        fn domains(&self) -> Vec<String> {
            vec![]
        }

        async fn handle(&self, _book: &Arc<EventBook>) -> SagaResult<Vec<CommandBook>> {
            Err(SagaError::Failed("intentional saga failure".to_string()))
        }

        fn is_synchronous(&self) -> bool {
            self.synchronous
        }
    }

    #[tokio::test]
    async fn test_new_bus_has_no_projectors_or_sagas() {
        let bus = InProcessEventBus::new();
        let book = Arc::new(make_event_book("orders"));

        let result = bus.publish(book).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_projector_receives_events() {
        let bus = InProcessEventBus::new();

        let shared_projector = Arc::new(CountingProjector::new("shared", vec![]));
        bus.projectors.write().await.push(shared_projector.clone());

        let book = Arc::new(make_event_book("orders"));
        bus.publish(book).await.unwrap();

        assert_eq!(shared_projector.call_count(), 1);
    }

    #[tokio::test]
    async fn test_projector_domain_filtering() {
        let bus = InProcessEventBus::new();

        let orders_projector = Arc::new(CountingProjector::new("orders_only", vec!["orders".to_string()]));
        let inventory_projector = Arc::new(CountingProjector::new("inventory_only", vec!["inventory".to_string()]));

        bus.projectors.write().await.push(orders_projector.clone());
        bus.projectors.write().await.push(inventory_projector.clone());

        let book = Arc::new(make_event_book("orders"));
        bus.publish(book).await.unwrap();

        assert_eq!(orders_projector.call_count(), 1);
        assert_eq!(inventory_projector.call_count(), 0);
    }

    #[tokio::test]
    async fn test_empty_domains_receives_all_events() {
        let bus = InProcessEventBus::new();

        let all_domains = Arc::new(CountingProjector::new("all", vec![]));
        bus.projectors.write().await.push(all_domains.clone());

        bus.publish(Arc::new(make_event_book("orders"))).await.unwrap();
        bus.publish(Arc::new(make_event_book("inventory"))).await.unwrap();

        assert_eq!(all_domains.call_count(), 2);
    }

    #[tokio::test]
    async fn test_saga_produces_commands() {
        let bus = InProcessEventBus::new();

        let saga = Arc::new(CommandProducingSaga::new(
            "producer",
            vec![],
            vec![CommandBook {
                cover: Some(Cover {
                    domain: "target".to_string(),
                    root: Some(ProtoUuid { value: vec![1; 16] }),
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(Any {
                        type_url: "test.Command".to_string(),
                        value: vec![],
                    }),
                    synchronous: false,
                }],
            }],
        ));
        bus.sagas.write().await.push(saga);

        bus.publish(Arc::new(make_event_book("orders"))).await.unwrap();

        let pending = bus.take_pending_commands().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].cover.as_ref().unwrap().domain,
            "target"
        );
    }

    #[tokio::test]
    async fn test_take_pending_commands_clears_buffer() {
        let bus = InProcessEventBus::new();

        let saga = Arc::new(CommandProducingSaga::new(
            "producer",
            vec![],
            vec![CommandBook {
                cover: Some(Cover {
                    domain: "target".to_string(),
                    root: Some(ProtoUuid { value: vec![1; 16] }),
                }),
                pages: vec![],
            }],
        ));
        bus.sagas.write().await.push(saga);

        bus.publish(Arc::new(make_event_book("orders"))).await.unwrap();

        let first = bus.take_pending_commands().await;
        assert_eq!(first.len(), 1);

        let second = bus.take_pending_commands().await;
        assert!(second.is_empty());
    }

    #[tokio::test]
    async fn test_synchronous_projector_failure_returns_error() {
        let bus = InProcessEventBus::new();

        let failing = Arc::new(FailingProjector { synchronous: true });
        bus.projectors.write().await.push(failing);

        let result = bus.publish(Arc::new(make_event_book("orders"))).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BusError::ProjectorFailed { name, .. } => {
                assert_eq!(name, "failing");
            }
            _ => panic!("Expected ProjectorFailed error"),
        }
    }

    #[tokio::test]
    async fn test_async_projector_failure_logs_but_continues() {
        let bus = InProcessEventBus::new();

        let failing = Arc::new(FailingProjector { synchronous: false });
        let counter = Arc::new(CountingProjector::new("counter", vec![]));

        bus.projectors.write().await.push(failing);
        bus.projectors.write().await.push(counter.clone());

        let result = bus.publish(Arc::new(make_event_book("orders"))).await;

        assert!(result.is_ok());
        assert_eq!(counter.call_count(), 1);
    }

    #[tokio::test]
    async fn test_synchronous_saga_failure_returns_error() {
        let bus = InProcessEventBus::new();

        let failing = Arc::new(FailingSaga { synchronous: true });
        bus.sagas.write().await.push(failing);

        let result = bus.publish(Arc::new(make_event_book("orders"))).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            BusError::SagaFailed { name, .. } => {
                assert_eq!(name, "failing_saga");
            }
            _ => panic!("Expected SagaFailed error"),
        }
    }

    #[tokio::test]
    async fn test_async_saga_failure_logs_but_continues() {
        let bus = InProcessEventBus::new();

        let failing = Arc::new(FailingSaga { synchronous: false });
        bus.sagas.write().await.push(failing);

        let result = bus.publish(Arc::new(make_event_book("orders"))).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_subscribe_not_supported() {
        let bus = InProcessEventBus::new();

        struct DummyHandler;
        impl EventHandler for DummyHandler {
            fn handle(
                &self,
                _book: Arc<EventBook>,
            ) -> futures::future::BoxFuture<'static, std::result::Result<(), BusError>> {
                Box::pin(async { Ok(()) })
            }
        }

        let result = bus.subscribe(Box::new(DummyHandler)).await;

        assert!(matches!(result, Err(BusError::SubscribeNotSupported)));
    }

    #[tokio::test]
    async fn test_default_creates_empty_bus() {
        let bus = InProcessEventBus::default();
        let book = Arc::new(make_event_book("orders"));

        let result = bus.publish(book).await;

        assert!(result.is_ok());
        assert!(bus.take_pending_commands().await.is_empty());
    }
}
