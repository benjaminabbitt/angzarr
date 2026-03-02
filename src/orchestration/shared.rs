//! Shared orchestration utilities for saga and process manager flows.
//!
//! Extracts common patterns: destination fetching, correlation ID propagation,
//! and simple command execution loops.

use tracing::{debug, error, warn};

use super::command::{CommandExecutor, CommandOutcome};
use super::destination::DestinationFetcher;
use crate::proto::{CommandBook, Cover, EventBook, SyncMode};

/// Fetch destination EventBooks for a set of covers.
///
/// Iterates covers and fetches state via the destination fetcher.
/// Skips (with warning) any covers that fail to resolve.
#[tracing::instrument(name = "orchestration.fetch", skip_all, fields(%correlation_id, count = covers.len()))]
pub async fn fetch_destinations(
    fetcher: &dyn DestinationFetcher,
    covers: &[Cover],
    correlation_id: &str,
) -> Vec<EventBook> {
    let mut destinations = Vec::with_capacity(covers.len());
    for cover in covers {
        if let Some(event_book) = fetcher.fetch(cover).await {
            destinations.push(event_book);
        } else {
            warn!(
                domain = %cover.domain,
                "Failed to fetch destination, skipping"
            );
        }
    }
    destinations
}

/// Ensure correlation_id is set on all command covers.
///
/// Fills in the correlation_id on any command whose cover has an empty one.
pub fn fill_correlation_id(commands: &mut [CommandBook], correlation_id: &str) {
    for command in commands.iter_mut() {
        if let Some(ref mut cover) = command.cover {
            if cover.correlation_id.is_empty() {
                cover.correlation_id = correlation_id.to_string();
            }
        }
    }
}

/// Execute commands sequentially, logging outcomes.
///
/// Used by process managers for simple fire-and-forget command execution.
/// Saga uses its own retry-aware execution loop instead.
///
/// `sync_mode` is passed to each command execution:
/// - `Cascade`: Sync execution, no bus publishing
/// - `Simple`/`Unspecified`: Standard execution with bus publishing
#[tracing::instrument(name = "orchestration.execute", skip_all, fields(%correlation_id, count = commands.len()))]
pub async fn execute_commands(
    executor: &dyn CommandExecutor,
    mut commands: Vec<CommandBook>,
    correlation_id: &str,
    sync_mode: SyncMode,
) {
    fill_correlation_id(&mut commands, correlation_id);

    for command_book in commands {
        let cmd_domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_else(|| "unknown".to_string());

        debug!(
            domain = %cmd_domain,
            "Executing command"
        );

        match executor.execute(command_book, sync_mode).await {
            CommandOutcome::Success(cmd_response) => {
                debug!(
                    domain = %cmd_domain,
                    has_events = cmd_response.events.is_some(),
                    "Command executed successfully"
                );
            }
            CommandOutcome::Retryable { reason, .. } => {
                warn!(
                    domain = %cmd_domain,
                    error = %reason,
                    "Command sequence conflict"
                );
            }
            CommandOutcome::Rejected(reason) => {
                error!(
                    domain = %cmd_domain,
                    error = %reason,
                    "Command failed (non-retryable)"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for fill_correlation_id.
    //!
    //! Correlation IDs enable cross-domain tracing in saga and process manager
    //! flows. When a saga produces commands, the framework must ensure each
    //! command carries the correlation ID from the triggering event — otherwise
    //! observability breaks and PMs cannot correlate related events.

    use super::*;

    fn make_command_with_correlation(domain: &str, correlation_id: &str) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                correlation_id: correlation_id.to_string(),
                ..Default::default()
            }),
            pages: vec![],
            saga_origin: None,
        }
    }

    /// Empty command list should not panic or produce side effects.
    #[test]
    fn test_fill_correlation_id_empty_commands() {
        let mut commands: Vec<CommandBook> = vec![];
        fill_correlation_id(&mut commands, "corr-123");
        assert!(commands.is_empty());
    }

    /// Commands with empty correlation_id should receive the propagated value.
    ///
    /// This is the primary use case: saga/PM produces commands without setting
    /// correlation_id, and the framework fills it in from the triggering event.
    #[test]
    fn test_fill_correlation_id_fills_empty() {
        let mut commands = vec![make_command_with_correlation("orders", "")];
        fill_correlation_id(&mut commands, "corr-123");

        assert_eq!(
            commands[0].cover.as_ref().unwrap().correlation_id,
            "corr-123"
        );
    }

    /// Commands that already have a correlation_id should not be overwritten.
    ///
    /// Sagas may explicitly set correlation_id when routing to a different
    /// workflow context. The framework must respect explicit values.
    #[test]
    fn test_fill_correlation_id_preserves_existing() {
        let mut commands = vec![make_command_with_correlation("orders", "existing-corr")];
        fill_correlation_id(&mut commands, "new-corr");

        assert_eq!(
            commands[0].cover.as_ref().unwrap().correlation_id,
            "existing-corr"
        );
    }

    /// Mixed batch: fill empty, preserve existing.
    ///
    /// Process managers may emit multiple commands to different domains.
    /// Some may have explicit correlation_ids (e.g., spawning a new workflow),
    /// while others should inherit the current workflow's correlation.
    #[test]
    fn test_fill_correlation_id_mixed() {
        let mut commands = vec![
            make_command_with_correlation("orders", ""),
            make_command_with_correlation("inventory", "existing"),
            make_command_with_correlation("fulfillment", ""),
        ];
        fill_correlation_id(&mut commands, "new-corr");

        assert_eq!(
            commands[0].cover.as_ref().unwrap().correlation_id,
            "new-corr"
        );
        assert_eq!(
            commands[1].cover.as_ref().unwrap().correlation_id,
            "existing"
        );
        assert_eq!(
            commands[2].cover.as_ref().unwrap().correlation_id,
            "new-corr"
        );
    }

    /// Commands without a cover should be skipped gracefully.
    ///
    /// Defensive: malformed commands shouldn't crash the framework.
    /// The router will reject them later with a proper error.
    #[test]
    fn test_fill_correlation_id_no_cover_skipped() {
        let mut commands = vec![
            make_command_with_correlation("orders", ""),
            CommandBook {
                cover: None,
                pages: vec![],
                saga_origin: None,
            },
        ];
        fill_correlation_id(&mut commands, "corr-123");

        assert_eq!(
            commands[0].cover.as_ref().unwrap().correlation_id,
            "corr-123"
        );
        assert!(commands[1].cover.is_none());
    }
}
