//! Aggregate command execution pipeline abstraction.
//!
//! `AggregateContext` trait + `execute_command_pipeline` for the shared
//! command execution flow (parse → load → validate → invoke → persist → publish).
//!
//! Business logic invocation is always via gRPC `AggregateClient`.
//! The trait covers storage access, post-persist behavior, and optional hooks.
//! - `local/`: SQLite-backed storage with static service discovery
//! - `grpc/`: Remote storage with K8s service discovery

// tonic::Status is large by design - it carries error details for gRPC
#![allow(clippy::result_large_err)]

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tonic::Status;
use tracing::debug;
use uuid::Uuid;

use crate::proto::{
    aggregate_client::AggregateClient, event_page, CommandBook, CommandResponse, ContextualCommand,
    EventBook, Projection,
};
use crate::utils::response_builder::extract_events_from_response;

/// How to load prior events.
#[derive(Debug, Clone)]
pub enum TemporalQuery {
    /// Current state (latest events, snapshot-optimized).
    Current,
    /// Events up to a specific sequence number (inclusive).
    AsOfSequence(u32),
    /// Events up to a specific timestamp.
    AsOfTimestamp(String),
}

/// Pipeline execution mode.
#[derive(Debug, Clone)]
pub enum PipelineMode {
    /// Normal execution: validate → invoke → persist → post-persist.
    Execute {
        /// Whether to validate command sequence against aggregate sequence.
        validate_sequence: bool,
    },
    /// Dry-run: load temporal state → invoke → return (no persist/publish).
    DryRun {
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<String>,
    },
}

/// Context for aggregate command pipeline.
///
/// Implementations provide storage access and post-persist behavior.
/// Business logic invocation is always via gRPC and handled by the pipeline.
#[async_trait]
pub trait AggregateContext: Send + Sync {
    /// Load prior events for the aggregate.
    async fn load_prior_events(
        &self,
        domain: &str,
        root: Uuid,
        temporal: &TemporalQuery,
    ) -> Result<EventBook, Status>;

    /// Persist new events to storage.
    async fn persist_events(
        &self,
        events: &EventBook,
        domain: &str,
        root: Uuid,
        correlation_id: &str,
    ) -> Result<EventBook, Status>;

    /// Publish to event bus AND call sync projectors via service discovery.
    /// Returns projections from sync projectors.
    async fn post_persist(&self, events: &EventBook) -> Result<Vec<Projection>, Status>;

    /// Optional: pre-validate sequence before loading events (gRPC fast-path).
    /// On mismatch, may return Status with EventBook in details.
    async fn pre_validate_sequence(
        &self,
        _domain: &str,
        _root: Uuid,
        _expected: u32,
    ) -> Result<(), Status> {
        Ok(())
    }

    /// Optional: transform events after loading (e.g., upcasting).
    async fn transform_events(
        &self,
        _domain: &str,
        events: EventBook,
    ) -> Result<EventBook, Status> {
        Ok(events)
    }
}

/// Parse domain and root UUID from a CommandBook cover.
pub fn parse_command_cover(command: &CommandBook) -> Result<(String, Uuid), Status> {
    let cover = command
        .cover
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("CommandBook must have a cover"))?;

    let domain = cover.domain.clone();
    let root = cover
        .root
        .as_ref()
        .ok_or_else(|| Status::invalid_argument("Cover must have a root UUID"))?;

    let root_uuid = Uuid::from_slice(&root.value)
        .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

    Ok((domain, root_uuid))
}

/// Extract expected sequence from the first command page.
pub fn extract_command_sequence(command: &CommandBook) -> u32 {
    command.pages.first().map(|p| p.sequence).unwrap_or(0)
}

/// Compute next expected sequence from an EventBook.
pub fn compute_next_sequence(events: &EventBook) -> u32 {
    events
        .pages
        .last()
        .and_then(|p| match &p.sequence {
            Some(event_page::Sequence::Num(n)) => Some(n + 1),
            _ => None,
        })
        .or_else(|| events.snapshot.as_ref().map(|s| s.sequence + 1))
        .unwrap_or(0)
}

/// Execute the aggregate command pipeline.
///
/// Flow:
/// - **Execute**: parse → correlation_id → pre-validate → load → transform →
///   validate sequence → invoke (gRPC) → persist → post-persist → response
/// - **DryRun**: parse → load temporal → transform → invoke (gRPC) → response (no persist)
pub async fn execute_command_pipeline(
    ctx: &dyn AggregateContext,
    business_client: &Mutex<AggregateClient<tonic::transport::Channel>>,
    command_book: CommandBook,
    mode: PipelineMode,
) -> Result<CommandResponse, Status> {
    let (domain, root_uuid) = parse_command_cover(&command_book)?;
    let correlation_id = crate::orchestration::correlation::ensure_correlation_id(&command_book)?;

    match mode {
        PipelineMode::Execute { validate_sequence } => {
            execute_mode(
                ctx,
                business_client,
                command_book,
                &domain,
                root_uuid,
                &correlation_id,
                validate_sequence,
            )
            .await
        }
        PipelineMode::DryRun {
            as_of_sequence,
            as_of_timestamp,
        } => {
            dry_run_mode(
                ctx,
                business_client,
                command_book,
                &domain,
                root_uuid,
                as_of_sequence,
                as_of_timestamp,
            )
            .await
        }
    }
}

async fn execute_mode(
    ctx: &dyn AggregateContext,
    business_client: &Mutex<AggregateClient<tonic::transport::Channel>>,
    command_book: CommandBook,
    domain: &str,
    root_uuid: Uuid,
    correlation_id: &str,
    validate_sequence: bool,
) -> Result<CommandResponse, Status> {
    debug!(
        domain = %domain,
        root = %root_uuid,
        correlation_id = %correlation_id,
        "Executing command"
    );

    // Pre-validate sequence (gRPC fast-path, no-op for local)
    if validate_sequence {
        let expected = extract_command_sequence(&command_book);
        ctx.pre_validate_sequence(domain, root_uuid, expected)
            .await?;
    }

    // Load prior events
    let prior_events = ctx
        .load_prior_events(domain, root_uuid, &TemporalQuery::Current)
        .await?;

    // Transform events (upcasting)
    let prior_events = ctx.transform_events(domain, prior_events).await?;

    // Validate command sequence against loaded events
    if validate_sequence {
        let expected = extract_command_sequence(&command_book);
        let actual = compute_next_sequence(&prior_events);
        if expected != actual {
            return Err(Status::failed_precondition(format!(
                "Sequence mismatch: command expects {expected}, aggregate at {actual}"
            )));
        }
    }

    // Invoke business logic via gRPC
    let contextual_command = ContextualCommand {
        events: Some(prior_events),
        command: Some(command_book),
    };

    let response = business_client
        .lock()
        .await
        .handle(contextual_command)
        .await?
        .into_inner();

    let new_events = extract_events_from_response(response, correlation_id.to_string())?;

    // Persist
    let persisted = ctx
        .persist_events(&new_events, domain, root_uuid, correlation_id)
        .await?;

    // Post-persist: publish + sync projectors
    let projections = ctx.post_persist(&persisted).await?;

    Ok(CommandResponse {
        events: Some(persisted),
        projections,
    })
}

async fn dry_run_mode(
    ctx: &dyn AggregateContext,
    business_client: &Mutex<AggregateClient<tonic::transport::Channel>>,
    command_book: CommandBook,
    domain: &str,
    root_uuid: Uuid,
    as_of_sequence: Option<u32>,
    as_of_timestamp: Option<String>,
) -> Result<CommandResponse, Status> {
    debug!(
        domain = %domain,
        root = %root_uuid,
        ?as_of_sequence,
        ?as_of_timestamp,
        "Dry-run command"
    );

    let temporal = match (as_of_sequence, as_of_timestamp) {
        (Some(seq), _) => TemporalQuery::AsOfSequence(seq),
        (_, Some(ts)) => TemporalQuery::AsOfTimestamp(ts),
        (None, None) => {
            return Err(Status::invalid_argument(
                "DryRun requires either as_of_sequence or as_of_timestamp",
            ));
        }
    };

    let prior_events = ctx.load_prior_events(domain, root_uuid, &temporal).await?;
    let prior_events = ctx.transform_events(domain, prior_events).await?;

    let contextual_command = ContextualCommand {
        events: Some(prior_events),
        command: Some(command_book),
    };

    let response = business_client
        .lock()
        .await
        .handle(contextual_command)
        .await?
        .into_inner();

    // For dry-run, extract events but don't set correlation_id (speculative)
    let speculative_events = extract_events_from_response(response, String::new())?;

    Ok(CommandResponse {
        events: Some(speculative_events),
        projections: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandPage, Cover, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_command_book(domain: &str, root: Uuid, sequence: u32) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            pages: vec![CommandPage {
                sequence,
                command: Some(Any {
                    type_url: "test.Command".to_string(),
                    value: vec![],
                }),
            }],
            saga_origin: None,
        }
    }

    fn make_event_book(domain: &str, root: Uuid, last_sequence: Option<u32>) -> EventBook {
        use crate::proto::EventPage;

        let pages = if let Some(seq) = last_sequence {
            vec![EventPage {
                sequence: Some(event_page::Sequence::Num(seq)),
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }]
        } else {
            vec![]
        };

        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            pages,
            snapshot: None,
            snapshot_state: None,
        }
    }

    #[test]
    fn test_parse_command_cover_valid() {
        let root = Uuid::new_v4();
        let command = make_command_book("orders", root, 0);

        let (domain, parsed_root) = parse_command_cover(&command).unwrap();

        assert_eq!(domain, "orders");
        assert_eq!(parsed_root, root);
    }

    #[test]
    fn test_parse_command_cover_missing_cover() {
        let command = CommandBook {
            cover: None,
            pages: vec![],
            saga_origin: None,
        };

        let result = parse_command_cover(&command);
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("cover"));
    }

    #[test]
    fn test_parse_command_cover_missing_root() {
        let command = CommandBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: None,
                correlation_id: String::new(),
            }),
            pages: vec![],
            saga_origin: None,
        };

        let result = parse_command_cover(&command);
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("root"));
    }

    #[test]
    fn test_extract_command_sequence() {
        let root = Uuid::new_v4();
        let command = make_command_book("orders", root, 5);

        assert_eq!(extract_command_sequence(&command), 5);
    }

    #[test]
    fn test_extract_command_sequence_empty_pages() {
        let command = CommandBook {
            cover: None,
            pages: vec![],
            saga_origin: None,
        };

        assert_eq!(extract_command_sequence(&command), 0);
    }

    #[test]
    fn test_compute_next_sequence_from_events() {
        let root = Uuid::new_v4();
        let events = make_event_book("orders", root, Some(4));

        assert_eq!(compute_next_sequence(&events), 5);
    }

    #[test]
    fn test_compute_next_sequence_empty_events() {
        let root = Uuid::new_v4();
        let events = make_event_book("orders", root, None);

        assert_eq!(compute_next_sequence(&events), 0);
    }

    #[test]
    fn test_compute_next_sequence_from_snapshot() {
        use crate::proto::Snapshot;

        let root = Uuid::new_v4();
        let mut events = make_event_book("orders", root, None);
        events.snapshot = Some(Snapshot {
            sequence: 10,
            state: None,
        });

        assert_eq!(compute_next_sequence(&events), 11);
    }
}
