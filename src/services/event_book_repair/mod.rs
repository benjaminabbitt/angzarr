//! EventBook completeness detection and repair.
//!
//! Provides utilities for detecting incomplete EventBooks and fetching
//! complete history from the EventQuery service.

use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tracing::{debug, info};
use uuid::Uuid;

use crate::proto::event_page::Sequence;
use crate::proto::{
    event_query_service_client::EventQueryServiceClient, EventBook, Query, Uuid as ProtoUuid,
};

/// Result type for repair operations.
pub type Result<T> = std::result::Result<T, RepairError>;

/// Errors that can occur during EventBook repair.
#[derive(Debug, thiserror::Error)]
pub enum RepairError {
    #[error("EventBook missing cover")]
    MissingCover,

    #[error("EventBook missing root UUID")]
    MissingRoot,

    #[error("Invalid UUID: {0}")]
    InvalidUuid(#[from] uuid::Error),

    #[error("gRPC error: {0}")]
    Grpc(Box<tonic::Status>),

    #[error("Transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    #[error("No EventBook returned from query")]
    NoEventBookReturned,
}

impl From<tonic::Status> for RepairError {
    fn from(status: tonic::Status) -> Self {
        RepairError::Grpc(Box::new(status))
    }
}

/// Check if an EventBook is complete.
///
/// An EventBook is considered complete if:
/// - It has a snapshot, OR
/// - Its first event has sequence 0
///
/// An empty EventBook (no events, no snapshot) is considered complete
/// for a new aggregate.
pub fn is_complete(book: &EventBook) -> bool {
    // Has snapshot - complete from snapshot onwards
    if book.snapshot.is_some() {
        return true;
    }

    // No events - empty aggregate, considered complete
    if book.pages.is_empty() {
        return true;
    }

    // Check if first event is sequence 0
    if let Some(first_page) = book.pages.first() {
        if let Some(ref seq) = first_page.sequence {
            match seq {
                Sequence::Num(n) => return *n == 0,
                Sequence::Force(_) => return true, // Force sequence is always valid
            }
        }
    }

    false
}

/// Extract domain and root UUID from an EventBook.
pub fn extract_identity(book: &EventBook) -> Result<(String, Uuid)> {
    let cover = book.cover.as_ref().ok_or(RepairError::MissingCover)?;
    let root = cover.root.as_ref().ok_or(RepairError::MissingRoot)?;
    let root_uuid = Uuid::from_slice(&root.value)?;
    Ok((cover.domain.clone(), root_uuid))
}

/// Fetch a complete EventBook from the EventQuery service.
///
/// Makes a synchronous gRPC call to fetch the full event history
/// for the given domain and root.
pub async fn fetch_complete(
    client: &mut EventQueryServiceClient<Channel>,
    domain: &str,
    root: Uuid,
) -> Result<EventBook> {
    let query = Query {
        cover: Some(crate::proto::Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        selection: None, // Full query - all events
    };

    let response = client.get_events(query).await?;
    let mut stream = response.into_inner();

    // GetEvents returns a stream with a single EventBook
    if let Some(result) = stream.next().await {
        let book = result?;
        debug!(
            domain = %domain,
            root = %root,
            events = book.pages.len(),
            has_snapshot = book.snapshot.is_some(),
            "Fetched complete EventBook"
        );
        Ok(book)
    } else {
        Err(RepairError::NoEventBookReturned)
    }
}

/// Repair an EventBook if incomplete.
///
/// If the EventBook is incomplete (missing history), fetches the complete
/// EventBook from the EventQuery service. Returns the original book if
/// already complete.
pub async fn repair_if_needed(
    client: &mut EventQueryServiceClient<Channel>,
    book: EventBook,
) -> Result<EventBook> {
    if is_complete(&book) {
        debug!("EventBook is complete, no repair needed");
        return Ok(book);
    }

    let (domain, root) = extract_identity(&book)?;
    info!(
        domain = %domain,
        root = %root,
        "EventBook incomplete, fetching complete history"
    );

    fetch_complete(client, &domain, root).await
}

/// Client wrapper for EventBook repair operations.
///
/// Maintains a connection to the EventQuery service and provides
/// convenient methods for repairing EventBooks.
pub struct EventBookRepairer {
    client: EventQueryServiceClient<Channel>,
}

impl EventBookRepairer {
    /// Create a new repairer connected to the given EventQuery service address.
    pub async fn connect(address: &str) -> Result<Self> {
        let channel = Channel::from_shared(format!("http://{}", address))
            .map_err(|e| RepairError::InvalidUri(e.to_string()))?
            .connect()
            .await?;

        Ok(Self {
            client: EventQueryServiceClient::new(channel),
        })
    }

    /// Create a new repairer from an existing channel.
    pub fn new(channel: Channel) -> Self {
        Self {
            client: EventQueryServiceClient::new(channel),
        }
    }

    /// Repair an EventBook if incomplete.
    pub async fn repair(&mut self, book: EventBook) -> Result<EventBook> {
        repair_if_needed(&mut self.client, book).await
    }

    /// Fetch a complete EventBook by domain and root.
    pub async fn fetch_full(&mut self, domain: &str, root: Uuid) -> Result<EventBook> {
        fetch_complete(&mut self.client, domain, root).await
    }

    /// Check if an EventBook is complete.
    pub fn is_complete(&self, book: &EventBook) -> bool {
        is_complete(book)
    }
}

#[cfg(test)]
mod tests;
