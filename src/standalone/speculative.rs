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
use crate::proto::{CommandBook, Cover, EventBook, EventPage, Projection, Uuid as ProtoUuid};

use super::router::DomainStorage;
use super::traits::{
    ProcessManagerHandler, ProjectionMode, ProjectorHandler, SagaHandler,
};

// ============================================================================
// Types
// ============================================================================

/// How to resolve state for a domain during speculative execution.
#[derive(Debug, Clone)]
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
    projectors: HashMap<String, Arc<dyn ProjectorHandler>>,
    sagas: HashMap<String, Arc<dyn SagaHandler>>,
    /// PM handler + PM's own domain name.
    process_managers: HashMap<String, (Arc<dyn ProcessManagerHandler>, String)>,
    domain_stores: HashMap<String, DomainStorage>,
}

impl SpeculativeExecutor {
    /// Create from cloned handler references and domain stores.
    pub fn new(
        projectors: HashMap<String, Arc<dyn ProjectorHandler>>,
        sagas: HashMap<String, Arc<dyn SagaHandler>>,
        process_managers: HashMap<String, (Arc<dyn ProcessManagerHandler>, String)>,
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
        let handler = self.projectors.get(name).ok_or_else(|| {
            Status::not_found(format!("No projector registered with name: {name}"))
        })?;

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
        let handler = self.sagas.get(name).ok_or_else(|| {
            Status::not_found(format!("No saga registered with name: {name}"))
        })?;

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
        let (handler, pm_domain) = self.process_managers.get(name).ok_or_else(|| {
            Status::not_found(format!("No process manager registered with name: {name}"))
        })?;

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
                .resolve_state(
                    pm_domain,
                    root,
                    &spec.unwrap_or(DomainStateSpec::Current),
                )
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

        // Phase 3: handle — produce commands + PM events
        let (commands, process_events) =
            handler.handle(trigger, pm_state.as_ref(), &destinations);

        Ok(PmSpeculativeResult {
            commands,
            process_events,
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
            Status::not_found(format!(
                "No storage configured for domain: {domain}"
            ))
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
                edition: Some(DEFAULT_EDITION.to_string()),
            }),
            snapshot: None,
            pages,
            snapshot_state: None,
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
    fn test_root_from_event_book_valid() {
        let root = Uuid::new_v4();
        let book = SpeculativeExecutor::build_event_book("order", root, vec![]);
        assert_eq!(
            SpeculativeExecutor::root_from_event_book(&book),
            Some(root)
        );
    }

    #[test]
    fn test_root_from_event_book_missing_cover() {
        let book = EventBook::default();
        assert_eq!(SpeculativeExecutor::root_from_event_book(&book), None);
    }

    #[test]
    fn test_domain_state_spec_variants() {
        // Verify all variants can be constructed
        let _ = DomainStateSpec::Current;
        let _ = DomainStateSpec::AtSequence(5);
        let _ = DomainStateSpec::AtTimestamp("2024-01-01T00:00:00Z".to_string());
        let _ = DomainStateSpec::Explicit(EventBook::default());
    }
}
