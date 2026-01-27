//! Event query service.

use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::proto::{
    event_query_server::EventQuery as EventQueryTrait, query::Selection,
    temporal_query::PointInTime, AggregateRoot, EventBook, Query, Uuid as ProtoUuid,
};
use crate::repository::EventBookRepository;
use crate::storage::EventStore;
use crate::storage::SnapshotStore;

/// Event query service.
///
/// Provides query access to the event store.
pub struct EventQueryService {
    event_book_repo: Arc<EventBookRepository>,
    event_store: Arc<dyn EventStore>,
}

impl EventQueryService {
    /// Create a new event query service with snapshot optimization enabled.
    ///
    /// Snapshots are enabled by default because sagas benefit from the
    /// optimization (snapshot + events after snapshot vs all events).
    pub fn new(event_store: Arc<dyn EventStore>, snapshot_store: Arc<dyn SnapshotStore>) -> Self {
        Self::with_options(event_store, snapshot_store, true)
    }

    /// Create a new event query service with configurable snapshot reading.
    ///
    /// Use `enable_snapshots = true` (default) for saga workloads where snapshots
    /// improve efficiency. Use `false` for raw event queries (debugging, replay).
    pub fn with_options(
        event_store: Arc<dyn EventStore>,
        snapshot_store: Arc<dyn SnapshotStore>,
        enable_snapshots: bool,
    ) -> Self {
        Self {
            event_book_repo: Arc::new(EventBookRepository::with_config(
                event_store.clone(),
                snapshot_store,
                enable_snapshots,
            )),
            event_store,
        }
    }
}

#[tonic::async_trait]
impl EventQueryTrait for EventQueryService {
    type GetEventsStream = ReceiverStream<Result<EventBook, Status>>;
    type SynchronizeStream = ReceiverStream<Result<EventBook, Status>>;
    type GetAggregateRootsStream = ReceiverStream<Result<AggregateRoot, Status>>;

    async fn get_event_book(&self, request: Request<Query>) -> Result<Response<EventBook>, Status> {
        let query = request.into_inner();
        let cover = query.cover.as_ref();

        // Extract correlation_id from cover
        let correlation_id = cover
            .map(|c| c.correlation_id.as_str())
            .unwrap_or("");

        // Correlation ID query: returns first matching EventBook across all domains
        // Useful for sagas that need to find related events without knowing the root ID
        if !correlation_id.is_empty() {
            info!(correlation_id = %correlation_id, "GetEventBook by correlation_id");

            let books = self
                .event_store
                .get_by_correlation(correlation_id)
                .await
                .map_err(|e| {
                    error!(correlation_id = %correlation_id, error = %e, "GetEventBook correlation query failed");
                    Status::internal(e.to_string())
                })?;

            // Return first matching book, or empty book if none found
            let book = books.into_iter().next().unwrap_or_default();
            info!(correlation_id = %correlation_id, pages = book.pages.len(), "GetEventBook by correlation_id completed");
            return Ok(Response::new(book));
        }

        // Standard query by domain + root
        let cover = cover.ok_or_else(|| {
            Status::invalid_argument("Query must have a cover with domain/root or correlation_id")
        })?;
        let domain = cover.domain.clone();
        let root = cover.root.as_ref().ok_or_else(|| {
            Status::invalid_argument("Query must have a root UUID or correlation_id")
        })?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;

        info!(
            domain = %domain,
            root = %root_uuid,
            selection = ?query.selection,
            "GetEventBook starting query"
        );

        // Handle selection: range, specific sequences, temporal, or full query
        let book = match query.selection {
            Some(Selection::Range(ref range)) => {
                let lower = range.lower;
                // Proto uses inclusive upper bound, storage uses exclusive.
                // Convert: inclusive N → exclusive N+1 (saturating to avoid overflow)
                let upper = range
                    .upper
                    .map(|u| u.saturating_add(1))
                    .unwrap_or(u32::MAX);
                info!(domain = %domain, root = %root_uuid, lower = lower, upper = upper, "GetEventBook range query");
                self.event_book_repo
                    .get_from_to(&domain, root_uuid, lower, upper)
                    .await
            }
            Some(Selection::Sequences(ref seq_set)) => {
                // TODO: Implement specific sequence fetching
                // For now, fall back to full query
                info!(domain = %domain, root = %root_uuid, sequences = ?seq_set.values, "GetEventBook sequences query (not yet implemented, using full)");
                self.event_book_repo.get(&domain, root_uuid).await
            }
            Some(Selection::Temporal(ref tq)) => {
                match tq.point_in_time {
                    Some(PointInTime::AsOfTime(ref ts)) => {
                        let rfc3339 = crate::storage::helpers::timestamp_to_rfc3339(ts)
                            .map_err(|e| Status::invalid_argument(e.to_string()))?;
                        info!(domain = %domain, root = %root_uuid, as_of = %rfc3339, "GetEventBook temporal time query");
                        self.event_book_repo
                            .get_temporal_by_time(&domain, root_uuid, &rfc3339)
                            .await
                    }
                    Some(PointInTime::AsOfSequence(seq)) => {
                        info!(domain = %domain, root = %root_uuid, as_of_sequence = seq, "GetEventBook temporal sequence query");
                        self.event_book_repo
                            .get_temporal_by_sequence(&domain, root_uuid, seq)
                            .await
                    }
                    None => {
                        return Err(Status::invalid_argument(
                            "TemporalQuery must specify as_of_time or as_of_sequence",
                        ));
                    }
                }
            }
            None => {
                info!(domain = %domain, root = %root_uuid, "GetEventBook full query");
                self.event_book_repo.get(&domain, root_uuid).await
            }
        }
        .map_err(|e| {
            error!(domain = %domain, root = %root_uuid, error = %e, "GetEventBook query failed");
            Status::internal(e.to_string())
        })?;

        info!(domain = %domain, root = %root_uuid, pages = book.pages.len(), "GetEventBook completed");
        Ok(Response::new(book))
    }

    async fn get_events(
        &self,
        request: Request<Query>,
    ) -> Result<Response<Self::GetEventsStream>, Status> {
        let query = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let cover = query.cover.as_ref();

        // Extract correlation_id from cover
        let correlation_id = cover
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();

        // Correlation ID query: streams ALL matching EventBooks across all domains
        if !correlation_id.is_empty() {
            let event_store = self.event_store.clone();

            tokio::spawn(async move {
                match event_store.get_by_correlation(&correlation_id).await {
                    Ok(books) => {
                        for book in books {
                            if tx.send(Ok(book)).await.is_err() {
                                break; // Client disconnected
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                    }
                }
            });

            return Ok(Response::new(ReceiverStream::new(rx)));
        }

        // Standard query by domain + root
        let cover = cover.ok_or_else(|| {
            Status::invalid_argument("Query must have a cover with domain/root or correlation_id")
        })?;
        let domain = cover.domain.clone();
        let root = cover.root.as_ref().ok_or_else(|| {
            Status::invalid_argument("Query must have a root UUID or correlation_id")
        })?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;

        let event_book_repo = self.event_book_repo.clone();

        tokio::spawn(async move {
            match event_book_repo.get(&domain, root_uuid).await {
                Ok(book) => {
                    let _ = tx.send(Ok(book)).await;
                }
                Err(e) => {
                    let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn synchronize(
        &self,
        request: Request<tonic::Streaming<Query>>,
    ) -> Result<Response<Self::SynchronizeStream>, Status> {
        let mut stream = request.into_inner();
        let event_book_repo = self.event_book_repo.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            use tokio_stream::StreamExt;

            while let Some(query_result) = stream.next().await {
                match query_result {
                    Ok(query) => {
                        let cover = match query.cover.as_ref() {
                            Some(c) => c,
                            None => {
                                let _ = tx
                                    .send(Err(Status::invalid_argument(
                                        "Query must have a cover",
                                    )))
                                    .await;
                                continue;
                            }
                        };
                        let domain = cover.domain.clone();
                        let root = match cover.root.as_ref() {
                            Some(r) => match uuid::Uuid::from_slice(&r.value) {
                                Ok(uuid) => uuid,
                                Err(e) => {
                                    error!(error = %e, "Invalid UUID in synchronize query");
                                    let _ = tx
                                        .send(Err(Status::invalid_argument(format!(
                                            "Invalid UUID: {e}"
                                        ))))
                                        .await;
                                    continue;
                                }
                            },
                            None => {
                                let _ = tx
                                    .send(Err(Status::invalid_argument(
                                        "Query must have a root UUID",
                                    )))
                                    .await;
                                continue;
                            }
                        };

                        // Handle selection: range, specific sequences, temporal, or full query
                        let result = match query.selection {
                            Some(Selection::Range(ref range)) => {
                                let lower = range.lower;
                                let upper = range.upper.unwrap_or(u32::MAX);
                                event_book_repo
                                    .get_from_to(&domain, root, lower, upper)
                                    .await
                            }
                            Some(Selection::Sequences(_)) => {
                                // TODO: Implement specific sequence fetching
                                event_book_repo.get(&domain, root).await
                            }
                            Some(Selection::Temporal(ref tq)) => {
                                match tq.point_in_time {
                                    Some(PointInTime::AsOfTime(ref ts)) => {
                                        match crate::storage::helpers::timestamp_to_rfc3339(ts) {
                                            Ok(rfc3339) => {
                                                event_book_repo
                                                    .get_temporal_by_time(&domain, root, &rfc3339)
                                                    .await
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(Err(Status::invalid_argument(
                                                        e.to_string(),
                                                    )))
                                                    .await;
                                                continue;
                                            }
                                        }
                                    }
                                    Some(PointInTime::AsOfSequence(seq)) => {
                                        event_book_repo
                                            .get_temporal_by_sequence(&domain, root, seq)
                                            .await
                                    }
                                    None => {
                                        let _ = tx
                                            .send(Err(Status::invalid_argument(
                                                "TemporalQuery must specify as_of_time or as_of_sequence",
                                            )))
                                            .await;
                                        continue;
                                    }
                                }
                            }
                            None => event_book_repo.get(&domain, root).await,
                        };

                        match result {
                            Ok(book) => {
                                info!(domain = %domain, root = %root, "Synchronize: sending event book");
                                if tx.send(Ok(book)).await.is_err() {
                                    break; // Client disconnected
                                }
                            }
                            Err(e) => {
                                error!(domain = %domain, root = %root, error = %e, "Synchronize: failed to get events");
                                if tx.send(Err(Status::internal(e.to_string()))).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Synchronize: stream error");
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn get_aggregate_roots(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::GetAggregateRootsStream>, Status> {
        let event_store = self.event_store.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            // Get all domains from the event store
            let domains = match event_store.list_domains().await {
                Ok(d) => d,
                Err(e) => {
                    error!(error = %e, "Failed to list domains");
                    let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                    return;
                }
            };

            for domain in domains {
                match event_store.list_roots(&domain).await {
                    Ok(roots) => {
                        for root in roots {
                            let aggregate = AggregateRoot {
                                domain: domain.clone(),
                                root: Some(ProtoUuid {
                                    value: root.as_bytes().to_vec(),
                                }),
                            };
                            if tx.send(Ok(aggregate)).await.is_err() {
                                return; // Client disconnected
                            }
                        }
                    }
                    Err(e) => {
                        error!(domain = %domain, error = %e, "Failed to list roots");
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, EventPage, SequenceRange, TemporalQuery};
    use crate::storage::mock::{MockEventStore, MockSnapshotStore};
    use prost_types::{Any, Timestamp};
    use tokio_stream::StreamExt;

    fn create_test_service_with_mocks(
        event_store: Arc<MockEventStore>,
        snapshot_store: Arc<MockSnapshotStore>,
    ) -> EventQueryService {
        EventQueryService::new(event_store, snapshot_store)
    }

    fn create_default_test_service() -> (
        EventQueryService,
        Arc<MockEventStore>,
        Arc<MockSnapshotStore>,
    ) {
        let event_store = Arc::new(MockEventStore::new());
        let snapshot_store = Arc::new(MockSnapshotStore::new());

        let service = create_test_service_with_mocks(event_store.clone(), snapshot_store.clone());

        (service, event_store, snapshot_store)
    }

    #[tokio::test]
    async fn test_get_event_book_empty_aggregate() {
        let (service, _, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert!(book.pages.is_empty());
    }

    #[tokio::test]
    async fn test_get_event_book_with_data() {
        let (service, event_store, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let events = vec![EventPage {
            sequence: Some(event_page::Sequence::Num(0)),
            event: Some(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }];
        event_store.add("orders", root, events, "").await.unwrap();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert_eq!(book.pages.len(), 1);
    }

    #[tokio::test]
    async fn test_get_event_book_missing_root() {
        let (service, _, _) = create_default_test_service();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: None,
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_get_event_book_invalid_uuid() {
        let (service, _, _) = create_default_test_service();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3], // Invalid UUID
                }),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_get_event_book_with_range() {
        let (service, event_store, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        // Add multiple events
        for i in 0..5 {
            let events = vec![EventPage {
                sequence: Some(event_page::Sequence::Num(i)),
                event: Some(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
            }];
            event_store.add("orders", root, events, "").await.unwrap();
        }

        // Query for range [2, 4] - inclusive bounds, should return events 2, 3, 4
        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: Some(Selection::Range(SequenceRange {
                lower: 2,
                upper: Some(4),
            })),
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert_eq!(book.pages.len(), 3); // Events 2, 3, 4 (inclusive upper bound)
    }

    #[tokio::test]
    async fn test_get_events_empty_aggregate() {
        let (service, _, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_events(Request::new(query)).await;

        assert!(response.is_ok());
        let mut stream = response.unwrap().into_inner();
        let first = stream.next().await;
        assert!(first.is_some());
        let book = first.unwrap().unwrap();
        assert!(book.pages.is_empty());
    }

    #[tokio::test]
    async fn test_get_events_with_data() {
        let (service, event_store, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        // First add some events via the store directly
        let events = vec![EventPage {
            sequence: Some(event_page::Sequence::Num(0)),
            event: Some(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }];
        event_store.add("orders", root, events, "").await.unwrap();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_events(Request::new(query)).await;

        assert!(response.is_ok());
        let mut stream = response.unwrap().into_inner();
        let first = stream.next().await;
        assert!(first.is_some());
        let book = first.unwrap().unwrap();
        assert_eq!(book.pages.len(), 1);
    }

    #[tokio::test]
    async fn test_get_events_missing_root() {
        let (service, _, _) = create_default_test_service();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: None,
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_events(Request::new(query)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_get_events_invalid_uuid() {
        let (service, _, _) = create_default_test_service();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: vec![1, 2, 3], // Invalid: must be 16 bytes
                }),
                correlation_id: String::new(),
            }),
            selection: None,
        };

        let response = service.get_events(Request::new(query)).await;

        assert!(response.is_err());
        let status = response.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
    }

    #[tokio::test]
    async fn test_get_aggregate_roots_empty() {
        let (service, _, _) = create_default_test_service();

        let response = service.get_aggregate_roots(Request::new(())).await;

        assert!(response.is_ok());
        let mut stream = response.unwrap().into_inner();
        let first = stream.next().await;
        assert!(first.is_none()); // No aggregates yet
    }

    #[tokio::test]
    async fn test_get_aggregate_roots_with_data() {
        let (service, event_store, _) = create_default_test_service();
        let root1 = uuid::Uuid::new_v4();
        let root2 = uuid::Uuid::new_v4();

        // Add some events
        event_store.add("orders", root1, vec![], "").await.unwrap();
        event_store.add("orders", root2, vec![], "").await.unwrap();

        let response = service.get_aggregate_roots(Request::new(())).await;

        assert!(response.is_ok());
        let stream = response.unwrap().into_inner();
        let roots: Vec<_> = stream.collect().await;
        assert_eq!(roots.len(), 2);
    }

    #[tokio::test]
    async fn test_get_aggregate_roots_multiple_domains() {
        let (service, event_store, _) = create_default_test_service();

        event_store
            .add("orders", uuid::Uuid::new_v4(), vec![], "")
            .await
            .unwrap();
        event_store
            .add("inventory", uuid::Uuid::new_v4(), vec![], "")
            .await
            .unwrap();

        let response = service.get_aggregate_roots(Request::new(())).await;

        assert!(response.is_ok());
        let stream = response.unwrap().into_inner();
        let roots: Vec<_> = stream.collect().await;
        assert_eq!(roots.len(), 2);
    }

    #[tokio::test]
    async fn test_get_event_book_by_correlation_id() {
        let (service, event_store, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();
        let correlation_id = "corr-123";

        // Add events with correlation ID
        let events = vec![EventPage {
            sequence: Some(event_page::Sequence::Num(0)),
            event: Some(Any {
                type_url: "test.Event".to_string(),
                value: vec![],
            }),
            created_at: None,
        }];
        event_store
            .add("orders", root, events, correlation_id)
            .await
            .unwrap();

        // Query by correlation ID (no root needed)
        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: String::new(),
                root: None,
                correlation_id: correlation_id.to_string(),
            }),
            selection: None,
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert_eq!(book.pages.len(), 1);
    }

    #[tokio::test]
    async fn test_get_event_book_by_correlation_id_not_found() {
        let (service, _, _) = create_default_test_service();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: String::new(),
                root: None,
                correlation_id: "nonexistent".to_string(),
            }),
            selection: None,
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert!(book.pages.is_empty());
    }

    #[tokio::test]
    async fn test_get_events_by_correlation_id_multiple_aggregates() {
        let (service, event_store, _) = create_default_test_service();
        let correlation_id = "corr-multi";

        // Add events to multiple aggregates with same correlation ID
        for (domain, root) in [
            ("orders", uuid::Uuid::new_v4()),
            ("inventory", uuid::Uuid::new_v4()),
        ] {
            let events = vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: format!("{}.Event", domain),
                    value: vec![],
                }),
                created_at: None,
            }];
            event_store
                .add(domain, root, events, correlation_id)
                .await
                .unwrap();
        }

        // Query by correlation ID - should return both
        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: String::new(),
                root: None,
                correlation_id: correlation_id.to_string(),
            }),
            selection: None,
        };

        let response = service.get_events(Request::new(query)).await;

        assert!(response.is_ok());
        let stream = response.unwrap().into_inner();
        let books: Vec<_> = stream.collect().await;
        assert_eq!(books.len(), 2);
    }

    #[tokio::test]
    async fn test_get_event_book_temporal_by_time() {
        let (service, event_store, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let events = vec![
            EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.Event0".to_string(),
                    value: vec![],
                }),
                created_at: Some(Timestamp {
                    seconds: 1704067200, // 2024-01-01T00:00:00Z
                    nanos: 0,
                }),
            },
            EventPage {
                sequence: Some(event_page::Sequence::Num(1)),
                event: Some(Any {
                    type_url: "test.Event1".to_string(),
                    value: vec![],
                }),
                created_at: Some(Timestamp {
                    seconds: 1704153600, // 2024-01-02T00:00:00Z
                    nanos: 0,
                }),
            },
            EventPage {
                sequence: Some(event_page::Sequence::Num(2)),
                event: Some(Any {
                    type_url: "test.Event2".to_string(),
                    value: vec![],
                }),
                created_at: Some(Timestamp {
                    seconds: 1704240000, // 2024-01-03T00:00:00Z
                    nanos: 0,
                }),
            },
        ];
        event_store.add("orders", root, events, "").await.unwrap();

        // Query as-of Jan 2
        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: Some(Selection::Temporal(TemporalQuery {
                point_in_time: Some(PointInTime::AsOfTime(Timestamp {
                    seconds: 1704153600, // 2024-01-02T00:00:00Z
                    nanos: 0,
                })),
            })),
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert_eq!(book.pages.len(), 2); // Events 0 and 1
        assert!(book.snapshot.is_none());
    }

    #[tokio::test]
    async fn test_get_event_book_temporal_by_sequence() {
        let (service, event_store, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        for i in 0..5 {
            let events = vec![EventPage {
                sequence: Some(event_page::Sequence::Num(i)),
                event: Some(Any {
                    type_url: format!("test.Event{}", i),
                    value: vec![],
                }),
                created_at: None,
            }];
            event_store.add("orders", root, events, "").await.unwrap();
        }

        // Query as-of sequence 2 — should return events 0, 1, 2
        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: Some(Selection::Temporal(TemporalQuery {
                point_in_time: Some(PointInTime::AsOfSequence(2)),
            })),
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_ok());
        let book = response.unwrap().into_inner();
        assert_eq!(book.pages.len(), 3);
        assert!(book.snapshot.is_none());
    }

    #[tokio::test]
    async fn test_get_event_book_temporal_empty_point_in_time() {
        let (service, _, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let query = Query {
            cover: Some(crate::proto::Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id: String::new(),
            }),
            selection: Some(Selection::Temporal(TemporalQuery {
                point_in_time: None,
            })),
        };

        let response = service.get_event_book(Request::new(query)).await;

        assert!(response.is_err());
        assert_eq!(response.unwrap_err().code(), tonic::Code::InvalidArgument);
    }
}
