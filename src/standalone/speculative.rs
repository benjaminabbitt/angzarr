//! Speculative execution for all component types.
//!
//! Runs handler logic without persistence, publishing, command execution,
//! or any side effects. Reuses the **same handler instances** registered
//! with the runtime — client logic is never duplicated. The framework
//! controls which side effects are suppressed; handlers are unaware of
//! the execution mode (except projectors, which receive `ProjectionMode`).
//!
//! Supported components:
//! - **Projectors**: returns `Projection` without persisting to read models
//! - **Sagas**: returns commands without executing them
//! - **Process managers**: returns commands + PM events without persistence

use std::collections::HashMap;
use std::sync::Arc;

use tonic::Status;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::{
    CommandBook, Cover, Edition, EventBook, EventPage, Projection, Uuid as ProtoUuid,
};

use super::router::DomainStorage;
use super::traits::{ProcessManagerHandler, ProjectionMode, ProjectorHandler, SagaHandler};

// Type aliases for complex handler maps to satisfy clippy::type_complexity
type ProjectorMap = HashMap<String, (Arc<dyn ProjectorHandler>, Vec<String>)>;
type SagaMap = HashMap<String, (Arc<dyn SagaHandler>, String)>;
type ProcessManagerMap = HashMap<
    String,
    (
        Arc<dyn ProcessManagerHandler>,
        String,
        Vec<crate::descriptor::Target>,
    ),
>;

// ============================================================================
// Types
// ============================================================================

/// How to resolve state for a domain during speculative execution.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum DomainStateSpec {
    /// Fetch current (latest) state from the event store.
    Current,
    /// Reconstruct state at a specific event sequence.
    AtSequence(u32),
    /// Reconstruct state up to a specific timestamp (RFC 3339).
    AtTimestamp(String),
    /// Use explicitly provided state (caller-supplied EventBook).
    Explicit(EventBook),
}

/// Result of speculative process manager execution.
#[derive(Debug)]
pub struct PmSpeculativeResult {
    /// Commands the PM would issue (not executed).
    pub commands: Vec<CommandBook>,
    /// PM events that would be persisted (not persisted).
    pub process_events: Option<EventBook>,
    /// Facts the PM would inject to other aggregates (not injected).
    pub facts: Vec<EventBook>,
}

// ============================================================================
// SpeculativeExecutor
// ============================================================================

/// Executes handler logic speculatively without side effects.
///
/// Holds `Arc` clones of the **same handler instances** used by the runtime.
/// Calling `speculate_*` invokes the same `prepare()`, `execute()`, `handle()`
/// methods as normal execution — only the surrounding framework behavior differs
/// (no persistence, no publishing, no command execution).
pub struct SpeculativeExecutor {
    /// Projector handler + subscribed domains (empty = all domains).
    projectors: ProjectorMap,
    /// Saga handler + input domain it subscribes to.
    sagas: SagaMap,
    /// PM handler + PM's own domain name + subscriptions.
    process_managers: ProcessManagerMap,
    domain_stores: HashMap<String, DomainStorage>,
}

impl SpeculativeExecutor {
    /// Create from cloned handler references and domain stores.
    pub fn new(
        projectors: ProjectorMap,
        sagas: SagaMap,
        process_managers: ProcessManagerMap,
        domain_stores: HashMap<String, DomainStorage>,
    ) -> Self {
        Self {
            projectors,
            sagas,
            process_managers,
            domain_stores,
        }
    }

    // ========================================================================
    // Public API
    // ========================================================================

    /// Speculatively run a projector against events.
    ///
    /// Invokes the same `ProjectorHandler::handle()` registered with the
    /// runtime, passing `ProjectionMode::Speculate` so the implementation
    /// skips persistence. Returns the `Projection` the handler computes.
    pub async fn speculate_projector(
        &self,
        name: &str,
        events: &EventBook,
    ) -> Result<Projection, Status> {
        let (handler, _domains) = self.projectors.get(name).ok_or_else(|| {
            Status::not_found(format!("No projector registered with name: {name}"))
        })?;

        handler.handle(events, ProjectionMode::Speculate).await
    }

    /// Speculatively run a projector for the given domain.
    ///
    /// Routes to the first projector that handles events from the specified domain.
    /// Used when the projector name is not explicitly provided (trait-based interface).
    pub async fn speculate_projector_by_domain(
        &self,
        domain: &str,
        events: &EventBook,
    ) -> Result<Projection, Status> {
        // Find a projector that handles this domain
        let (name, (handler, _)) = self
            .projectors
            .iter()
            .find(|(_, (_, domains))| {
                // Empty domains list means "all domains"
                domains.is_empty() || domains.iter().any(|d| d == domain)
            })
            .ok_or_else(|| {
                Status::not_found(format!("No projector registered for domain: {domain}"))
            })?;

        debug!(projector = %name, %domain, "Routing speculative projector by domain");
        handler.handle(events, ProjectionMode::Speculate).await
    }

    /// Speculatively run a saga against source events.
    ///
    /// Invokes the same `SagaHandler::prepare()` and `execute()` registered
    /// with the runtime. Destination state is resolved via `domain_specs`.
    /// Returns the commands the saga would produce without executing them.
    pub async fn speculate_saga(
        &self,
        name: &str,
        source: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<Vec<CommandBook>, Status> {
        let (handler, _input_domain) = self
            .sagas
            .get(name)
            .ok_or_else(|| Status::not_found(format!("No saga registered with name: {name}")))?;

        self.execute_saga_speculative(handler.as_ref(), source, domain_specs)
            .await
    }

    /// Speculatively run a saga for the given source domain.
    ///
    /// Routes to the first saga that handles events from the specified domain.
    /// Used when the saga name is not explicitly provided (trait-based interface).
    pub async fn speculate_saga_by_source_domain(
        &self,
        source_domain: &str,
        source: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<Vec<CommandBook>, Status> {
        // Find a saga that handles this source domain
        let (name, (handler, _)) = self
            .sagas
            .iter()
            .find(|(_, (_, input_domain))| input_domain == source_domain)
            .ok_or_else(|| {
                Status::not_found(format!(
                    "No saga registered for source domain: {source_domain}"
                ))
            })?;

        debug!(saga = %name, source_domain = %source_domain, "Routing speculative saga by domain");
        self.execute_saga_speculative(handler.as_ref(), source, domain_specs)
            .await
    }

    /// Execute saga speculatively (shared implementation).
    async fn execute_saga_speculative(
        &self,
        handler: &dyn SagaHandler,
        source: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<Vec<CommandBook>, Status> {
        // Phase 1: prepare — declare destination covers
        let covers = handler.prepare(source).await?;

        // Phase 2: resolve destinations
        let destinations = self.resolve_destinations(&covers, domain_specs).await?;

        // Phase 3: execute — produce commands
        let response = handler.execute(source, &destinations).await?;

        Ok(response.commands)
    }

    /// Speculatively run a process manager against a trigger event.
    ///
    /// Invokes the same `ProcessManagerHandler::prepare()` and `handle()`
    /// registered with the runtime. PM state and destination states are
    /// resolved via `domain_specs`. Returns commands + PM events without
    /// persisting PM events or executing commands.
    pub async fn speculate_pm(
        &self,
        name: &str,
        trigger: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<PmSpeculativeResult, Status> {
        let (handler, pm_domain, _subscriptions) =
            self.process_managers.get(name).ok_or_else(|| {
                Status::not_found(format!("No process manager registered with name: {name}"))
            })?;

        self.execute_pm_speculative(name, handler.as_ref(), pm_domain, trigger, domain_specs)
            .await
    }

    /// Speculatively run a process manager for the given trigger domain.
    ///
    /// Routes to the first PM that subscribes to events from the specified domain.
    /// Used when the PM name is not explicitly provided (trait-based interface).
    pub async fn speculate_pm_by_trigger_domain(
        &self,
        trigger_domain: &str,
        trigger: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<PmSpeculativeResult, Status> {
        // Find a PM that handles this trigger domain by checking subscriptions
        let (name, (handler, pm_domain, _)) = self
            .process_managers
            .iter()
            .find(|(_, (_, _, subscriptions))| {
                subscriptions.iter().any(|t| t.domain == trigger_domain)
            })
            .ok_or_else(|| {
                Status::not_found(format!(
                    "No process manager registered for trigger domain: {trigger_domain}"
                ))
            })?;

        debug!(pm = %name, trigger_domain = %trigger_domain, "Routing speculative PM by domain");
        self.execute_pm_speculative(name, handler.as_ref(), pm_domain, trigger, domain_specs)
            .await
    }

    /// Execute PM speculatively (shared implementation).
    async fn execute_pm_speculative(
        &self,
        name: &str,
        handler: &dyn ProcessManagerHandler,
        pm_domain: &str,
        trigger: &EventBook,
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<PmSpeculativeResult, Status> {
        // Resolve PM's own state
        let pm_root = Self::root_from_event_book(trigger);
        let pm_state = if let Some(root) = pm_root {
            let spec = domain_specs.get(pm_domain).cloned();
            if spec.is_none() {
                debug!(
                    domain = %pm_domain,
                    pm = %name,
                    "PM domain not in domain_specs, using current state"
                );
            }
            let resolved = self
                .resolve_state(pm_domain, root, &spec.unwrap_or(DomainStateSpec::Current))
                .await?;
            if resolved.pages.is_empty() {
                None
            } else {
                Some(resolved)
            }
        } else {
            None
        };

        // Phase 1: prepare — declare destination covers
        let covers = handler.prepare(trigger, pm_state.as_ref());

        // Log mismatches between prepare covers and domain_specs
        for cover in &covers {
            if !domain_specs.contains_key(&cover.domain) {
                warn!(
                    pm = %name,
                    domain = %cover.domain,
                    "PM prepare requested domain not in provided domain_specs, falling back to current state"
                );
            }
        }
        for spec_domain in domain_specs.keys() {
            let requested = covers.iter().any(|c| &c.domain == spec_domain);
            let is_pm_domain = spec_domain == pm_domain;
            if !requested && !is_pm_domain {
                debug!(
                    domain = %spec_domain,
                    pm = %name,
                    "domain_specs entry was not requested by PM prepare phase"
                );
            }
        }

        // Phase 2: resolve destinations
        let destinations = self.resolve_destinations(&covers, domain_specs).await?;

        // Phase 3: handle — produce commands + PM events + facts
        let result = handler.handle(trigger, pm_state.as_ref(), &destinations);

        Ok(PmSpeculativeResult {
            commands: result.commands,
            process_events: result.process_events,
            facts: result.facts,
        })
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Resolve aggregate state for a single domain/root according to a spec.
    async fn resolve_state(
        &self,
        domain: &str,
        root: Uuid,
        spec: &DomainStateSpec,
    ) -> Result<EventBook, Status> {
        let storage = self.domain_stores.get(domain).ok_or_else(|| {
            Status::not_found(format!("No storage configured for domain: {domain}"))
        })?;

        match spec {
            DomainStateSpec::Current => {
                let pages = storage
                    .event_store
                    .get(domain, DEFAULT_EDITION, root)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
                Ok(Self::build_event_book(domain, root, pages))
            }
            DomainStateSpec::AtSequence(seq) => {
                let pages = storage
                    .event_store
                    .get_from_to(domain, DEFAULT_EDITION, root, 0, seq.saturating_add(1))
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
                let actual = pages.len() as u32;
                if actual != seq.saturating_add(1) {
                    info!(
                        %domain,
                        requested_sequence = %seq,
                        actual_events = %actual,
                        "resolved state differs from requested sequence"
                    );
                }
                Ok(Self::build_event_book(domain, root, pages))
            }
            DomainStateSpec::AtTimestamp(ts) => {
                let pages = storage
                    .event_store
                    .get_until_timestamp(domain, DEFAULT_EDITION, root, ts)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
                Ok(Self::build_event_book(domain, root, pages))
            }
            DomainStateSpec::Explicit(book) => Ok(book.clone()),
        }
    }

    /// Resolve destination covers against domain_specs, falling back to Current with a warning.
    async fn resolve_destinations(
        &self,
        covers: &[Cover],
        domain_specs: &HashMap<String, DomainStateSpec>,
    ) -> Result<Vec<EventBook>, Status> {
        let mut destinations = Vec::with_capacity(covers.len());

        for cover in covers {
            let root = cover
                .root
                .as_ref()
                .and_then(|r| Uuid::from_slice(&r.value).ok())
                .ok_or_else(|| {
                    Status::invalid_argument(format!(
                        "Invalid root UUID in cover for domain: {}",
                        cover.domain
                    ))
                })?;

            let spec = match domain_specs.get(&cover.domain) {
                Some(s) => s,
                None => {
                    warn!(
                        domain = %cover.domain,
                        "domain not in provided domain_specs, falling back to current state"
                    );
                    &DomainStateSpec::Current
                }
            };

            let book = self.resolve_state(&cover.domain, root, spec).await?;
            destinations.push(book);
        }

        Ok(destinations)
    }

    /// Build an EventBook from domain, root, and pages.
    fn build_event_book(domain: &str, root: Uuid, pages: Vec<EventPage>) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
                edition: Some(Edition {
                    name: DEFAULT_EDITION.to_string(),
                    divergences: vec![],
                }),
                external_id: String::new(),
            }),
            snapshot: None,
            pages,
            ..Default::default()
        }
    }

    /// Extract root UUID from an EventBook's cover.
    fn root_from_event_book(book: &EventBook) -> Option<Uuid> {
        book.cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .and_then(|r| Uuid::from_slice(&r.value).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::event_page;
    use crate::proto_ext::EventPageExt;

    // ============================================================================
    // DomainStateSpec Tests
    // ============================================================================

    #[test]
    fn test_domain_state_spec_current() {
        let spec = DomainStateSpec::Current;
        // Current should be debug-printable
        assert!(format!("{:?}", spec).contains("Current"));
    }

    #[test]
    fn test_domain_state_spec_at_sequence() {
        let spec = DomainStateSpec::AtSequence(42);
        assert!(format!("{:?}", spec).contains("42"));
    }

    #[test]
    fn test_domain_state_spec_at_timestamp() {
        let ts = "2024-01-15T10:30:00Z".to_string();
        let spec = DomainStateSpec::AtTimestamp(ts.clone());
        assert!(format!("{:?}", spec).contains(&ts));
    }

    #[test]
    fn test_domain_state_spec_explicit() {
        let book = EventBook::default();
        let spec = DomainStateSpec::Explicit(book);
        assert!(format!("{:?}", spec).contains("Explicit"));
    }

    #[test]
    fn test_domain_state_spec_clone() {
        let spec = DomainStateSpec::AtSequence(10);
        let cloned = spec.clone();
        assert!(format!("{:?}", cloned).contains("10"));
    }

    // ============================================================================
    // PmSpeculativeResult Tests
    // ============================================================================

    #[test]
    fn test_pm_speculative_result_empty() {
        let result = PmSpeculativeResult {
            commands: vec![],
            process_events: None,
            facts: vec![],
        };

        assert!(result.commands.is_empty());
        assert!(result.process_events.is_none());
        assert!(result.facts.is_empty());
    }

    #[test]
    fn test_pm_speculative_result_with_commands() {
        let cmd = CommandBook::default();
        let result = PmSpeculativeResult {
            commands: vec![cmd],
            process_events: None,
            facts: vec![],
        };

        assert_eq!(result.commands.len(), 1);
    }

    #[test]
    fn test_pm_speculative_result_with_events() {
        let events = EventBook::default();
        let result = PmSpeculativeResult {
            commands: vec![],
            process_events: Some(events),
            facts: vec![],
        };

        assert!(result.process_events.is_some());
    }

    #[test]
    fn test_pm_speculative_result_with_facts() {
        let fact = EventBook::default();
        let result = PmSpeculativeResult {
            commands: vec![],
            process_events: None,
            facts: vec![fact],
        };

        assert_eq!(result.facts.len(), 1);
    }

    #[test]
    fn test_pm_speculative_result_debug() {
        let result = PmSpeculativeResult {
            commands: vec![],
            process_events: None,
            facts: vec![],
        };
        // Should be Debug-printable
        assert!(format!("{:?}", result).contains("PmSpeculativeResult"));
    }

    // ============================================================================
    // SpeculativeExecutor::build_event_book Tests
    // ============================================================================

    #[test]
    fn test_build_event_book_structure() {
        let root = Uuid::new_v4();
        let book = SpeculativeExecutor::build_event_book("order", root, vec![]);

        let cover = book.cover.as_ref().unwrap();
        assert_eq!(cover.domain, "order");
        assert_eq!(
            Uuid::from_slice(&cover.root.as_ref().unwrap().value).unwrap(),
            root
        );
        assert!(book.pages.is_empty());
        assert!(book.snapshot.is_none());
    }

    #[test]
    fn test_build_event_book_with_pages() {
        let root = Uuid::new_v4();
        let page = EventPage {
            sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(0)),
            payload: Some(event_page::Payload::Event(prost_types::Any {
                type_url: "test.Event".to_string(),
                value: vec![1, 2, 3],
            })),
            created_at: None,
        };
        let book = SpeculativeExecutor::build_event_book("order", root, vec![page]);

        assert_eq!(book.pages.len(), 1);
        assert_eq!(book.pages[0].sequence_num(), 0);
    }

    #[test]
    fn test_build_event_book_multiple_pages() {
        let root = Uuid::new_v4();
        let pages: Vec<EventPage> = (0..5)
            .map(|seq| EventPage {
                sequence_type: Some(event_page::SequenceType::Sequence(seq)),
                payload: Some(event_page::Payload::Event(prost_types::Any {
                    type_url: format!("test.Event{}", seq),
                    value: vec![],
                })),
                created_at: None,
            })
            .collect();

        let book = SpeculativeExecutor::build_event_book("order", root, pages);

        assert_eq!(book.pages.len(), 5);
        for (i, page) in book.pages.iter().enumerate() {
            assert_eq!(page.sequence_num(), i as u32);
        }
    }

    #[test]
    fn test_build_event_book_has_default_edition() {
        let root = Uuid::new_v4();
        let book = SpeculativeExecutor::build_event_book("order", root, vec![]);

        let cover = book.cover.as_ref().unwrap();
        let edition = cover.edition.as_ref().unwrap();
        assert_eq!(edition.name, DEFAULT_EDITION);
        assert!(edition.divergences.is_empty());
    }

    #[test]
    fn test_build_event_book_empty_correlation_id() {
        let root = Uuid::new_v4();
        let book = SpeculativeExecutor::build_event_book("order", root, vec![]);

        let cover = book.cover.as_ref().unwrap();
        assert!(cover.correlation_id.is_empty());
    }

    // ============================================================================
    // SpeculativeExecutor::root_from_event_book Tests
    // ============================================================================

    #[test]
    fn test_root_from_event_book_valid() {
        let root = Uuid::new_v4();
        let book = SpeculativeExecutor::build_event_book("order", root, vec![]);
        assert_eq!(SpeculativeExecutor::root_from_event_book(&book), Some(root));
    }

    #[test]
    fn test_root_from_event_book_missing_cover() {
        let book = EventBook::default();
        assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
    }

    #[test]
    fn test_root_from_event_book_missing_root() {
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: None,
                correlation_id: String::new(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };
        assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
    }

    #[test]
    fn test_root_from_event_book_invalid_uuid_bytes() {
        let book = EventBook {
            cover: Some(Cover {
                domain: "test".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3], // Invalid - not 16 bytes
                }),
                correlation_id: String::new(),
                edition: None,
                external_id: String::new(),
            }),
            pages: vec![],
            snapshot: None,
            ..Default::default()
        };
        assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
    }

    // ============================================================================
    // SpeculativeExecutor Construction Tests
    // ============================================================================

    #[test]
    fn test_speculative_executor_empty_construction() {
        let executor = SpeculativeExecutor::new(
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );

        // Should construct without error
        let _ = executor;
    }

    #[test]
    fn test_speculative_executor_with_domain_stores() {
        use crate::standalone::router::DomainStorage;
        use crate::storage::mock::{MockEventStore, MockSnapshotStore};

        let mut domain_stores = HashMap::new();
        domain_stores.insert(
            "orders".to_string(),
            DomainStorage {
                event_store: Arc::new(MockEventStore::new()),
                snapshot_store: Arc::new(MockSnapshotStore::new()),
            },
        );

        let executor = SpeculativeExecutor::new(
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
            domain_stores,
        );

        let _ = executor;
    }

    // ============================================================================
    // Edge Cases
    // ============================================================================

    #[test]
    fn test_domain_state_spec_zero_sequence() {
        let spec = DomainStateSpec::AtSequence(0);
        assert!(format!("{:?}", spec).contains("0"));
    }

    #[test]
    fn test_domain_state_spec_max_sequence() {
        let spec = DomainStateSpec::AtSequence(u32::MAX);
        assert!(format!("{:?}", spec).contains(&u32::MAX.to_string()));
    }

    #[test]
    fn test_domain_state_spec_empty_timestamp() {
        let spec = DomainStateSpec::AtTimestamp(String::new());
        // Empty timestamp is valid at this level (validation happens elsewhere)
        assert!(format!("{:?}", spec).contains("AtTimestamp"));
    }

    #[test]
    fn test_build_event_book_preserves_page_order() {
        let root = Uuid::new_v4();
        let pages: Vec<EventPage> = vec![
            EventPage {
                sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(5)),
                payload: None,
                created_at: None,
            },
            EventPage {
                sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(3)),
                payload: None,
                created_at: None,
            },
            EventPage {
                sequence_type: Some(crate::proto::event_page::SequenceType::Sequence(7)),
                payload: None,
                created_at: None,
            },
        ];

        let book = SpeculativeExecutor::build_event_book("order", root, pages);

        // Pages should be preserved in insertion order, not sorted
        assert_eq!(book.pages[0].sequence_num(), 5);
        assert_eq!(book.pages[1].sequence_num(), 3);
        assert_eq!(book.pages[2].sequence_num(), 7);
    }

    // ============================================================================
    // Domain Routing Tests
    // ============================================================================

    mod domain_routing {
        use super::*;
        use crate::descriptor::Target;
        use crate::orchestration::projector::ProjectionMode;
        use crate::proto::{Cover, Projection, SagaResponse};
        use crate::standalone::traits::{ProcessManagerHandleResult, ProcessManagerHandler};
        use async_trait::async_trait;

        /// Mock projector that always returns an empty projection.
        struct MockProjector;

        #[async_trait]
        impl super::super::ProjectorHandler for MockProjector {
            async fn handle(
                &self,
                _events: &EventBook,
                _mode: ProjectionMode,
            ) -> Result<Projection, Status> {
                Ok(Projection::default())
            }
        }

        /// Mock saga that returns empty response.
        struct MockSaga;

        #[async_trait]
        impl super::super::SagaHandler for MockSaga {
            async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
                Ok(vec![])
            }

            async fn execute(
                &self,
                _source: &EventBook,
                _destinations: &[EventBook],
            ) -> Result<SagaResponse, Status> {
                Ok(SagaResponse::default())
            }
        }

        /// Mock process manager that returns empty result.
        struct MockPM;

        impl ProcessManagerHandler for MockPM {
            fn prepare(
                &self,
                _trigger: &EventBook,
                _process_state: Option<&EventBook>,
            ) -> Vec<Cover> {
                vec![]
            }

            fn handle(
                &self,
                _trigger: &EventBook,
                _process_state: Option<&EventBook>,
                _destinations: &[EventBook],
            ) -> ProcessManagerHandleResult {
                ProcessManagerHandleResult {
                    commands: vec![],
                    process_events: None,
                    facts: vec![],
                }
            }
        }

        // --------------------------------------------------------------------
        // Projector Domain Routing
        // --------------------------------------------------------------------

        #[tokio::test]
        async fn test_projector_routing_finds_by_matching_domain() {
            let mut projectors = HashMap::new();
            projectors.insert(
                "orders-projector".to_string(),
                (
                    Arc::new(MockProjector) as Arc<dyn super::super::ProjectorHandler>,
                    vec!["orders".to_string()],
                ),
            );

            let executor = SpeculativeExecutor::new(
                projectors,
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            );

            let events = EventBook::default();
            let result = executor
                .speculate_projector_by_domain("orders", &events)
                .await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_projector_routing_empty_domains_matches_all() {
            let mut projectors = HashMap::new();
            projectors.insert(
                "catch-all-projector".to_string(),
                (
                    Arc::new(MockProjector) as Arc<dyn super::super::ProjectorHandler>,
                    vec![], // Empty = matches all domains
                ),
            );

            let executor = SpeculativeExecutor::new(
                projectors,
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            );

            let events = EventBook::default();

            // Should match any domain
            assert!(executor
                .speculate_projector_by_domain("orders", &events)
                .await
                .is_ok());
            assert!(executor
                .speculate_projector_by_domain("inventory", &events)
                .await
                .is_ok());
        }

        #[tokio::test]
        async fn test_projector_routing_not_found_for_unmatched_domain() {
            let mut projectors = HashMap::new();
            projectors.insert(
                "orders-projector".to_string(),
                (
                    Arc::new(MockProjector) as Arc<dyn super::super::ProjectorHandler>,
                    vec!["orders".to_string()],
                ),
            );

            let executor = SpeculativeExecutor::new(
                projectors,
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            );

            let events = EventBook::default();
            let result = executor
                .speculate_projector_by_domain("inventory", &events)
                .await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), tonic::Code::NotFound);
        }

        // --------------------------------------------------------------------
        // Saga Domain Routing
        // --------------------------------------------------------------------

        #[tokio::test]
        async fn test_saga_routing_finds_by_input_domain() {
            let mut sagas = HashMap::new();
            sagas.insert(
                "order-fulfillment".to_string(),
                (
                    Arc::new(MockSaga) as Arc<dyn super::super::SagaHandler>,
                    "orders".to_string(), // input_domain
                ),
            );

            let executor =
                SpeculativeExecutor::new(HashMap::new(), sagas, HashMap::new(), HashMap::new());

            let source = EventBook::default();
            let result = executor
                .speculate_saga_by_source_domain("orders", &source, &HashMap::new())
                .await;

            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_saga_routing_not_found_for_unmatched_domain() {
            let mut sagas = HashMap::new();
            sagas.insert(
                "order-fulfillment".to_string(),
                (
                    Arc::new(MockSaga) as Arc<dyn super::super::SagaHandler>,
                    "orders".to_string(),
                ),
            );

            let executor =
                SpeculativeExecutor::new(HashMap::new(), sagas, HashMap::new(), HashMap::new());

            let source = EventBook::default();
            let result = executor
                .speculate_saga_by_source_domain("inventory", &source, &HashMap::new())
                .await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), tonic::Code::NotFound);
        }

        // --------------------------------------------------------------------
        // Process Manager Domain Routing
        // --------------------------------------------------------------------

        #[tokio::test]
        async fn test_pm_routing_finds_by_subscription_domain() {
            let mut pms = HashMap::new();
            pms.insert(
                "order-flow".to_string(),
                (
                    Arc::new(MockPM) as Arc<dyn ProcessManagerHandler>,
                    "order-flow".to_string(), // PM domain
                    vec![Target::domain("orders"), Target::domain("inventory")], // subscriptions
                ),
            );

            let executor =
                SpeculativeExecutor::new(HashMap::new(), HashMap::new(), pms, HashMap::new());

            let trigger = EventBook::default();

            // Should find PM for subscribed domain
            let result = executor
                .speculate_pm_by_trigger_domain("orders", &trigger, &HashMap::new())
                .await;
            assert!(result.is_ok());

            let result = executor
                .speculate_pm_by_trigger_domain("inventory", &trigger, &HashMap::new())
                .await;
            assert!(result.is_ok());
        }

        #[tokio::test]
        async fn test_pm_routing_not_found_for_unsubscribed_domain() {
            let mut pms = HashMap::new();
            pms.insert(
                "order-flow".to_string(),
                (
                    Arc::new(MockPM) as Arc<dyn ProcessManagerHandler>,
                    "order-flow".to_string(),
                    vec![Target::domain("orders")], // Only subscribed to orders
                ),
            );

            let executor =
                SpeculativeExecutor::new(HashMap::new(), HashMap::new(), pms, HashMap::new());

            let trigger = EventBook::default();
            let result = executor
                .speculate_pm_by_trigger_domain("fulfillment", &trigger, &HashMap::new())
                .await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), tonic::Code::NotFound);
        }

        #[tokio::test]
        async fn test_pm_routing_empty_pms_returns_not_found() {
            let executor = SpeculativeExecutor::new(
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            );

            let trigger = EventBook::default();
            let result = executor
                .speculate_pm_by_trigger_domain("orders", &trigger, &HashMap::new())
                .await;

            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code(), tonic::Code::NotFound);
        }
    }
}
