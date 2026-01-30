//! Shared orchestration utilities for saga and process manager flows.
//!
//! Extracts common patterns: destination fetching, correlation ID propagation,
//! and simple command execution loops.

use tracing::{debug, error, warn};

use super::command::{CommandExecutor, CommandOutcome};
use super::destination::DestinationFetcher;
use crate::proto::{CommandBook, Cover, EventBook};

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
#[tracing::instrument(name = "orchestration.execute", skip_all, fields(%correlation_id, count = commands.len()))]
pub async fn execute_commands(
    executor: &dyn CommandExecutor,
    mut commands: Vec<CommandBook>,
    correlation_id: &str,
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

        match executor.execute(command_book).await {
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
