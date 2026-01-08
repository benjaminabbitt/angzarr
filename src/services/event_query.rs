//! Event query service.

use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{error, info};

use crate::interfaces::EventStore;
use crate::interfaces::SnapshotStore;
use crate::proto::{
    event_query_server::EventQuery as EventQueryTrait, AggregateRoot, EventBook, Query,
    Uuid as ProtoUuid,
};
use crate::repository::EventBookRepository;

/// Event query service.
///
/// Provides query access to the event store.
pub struct EventQueryService {
    event_book_repo: Arc<EventBookRepository>,
    event_store: Arc<dyn EventStore>,
}

impl EventQueryService {
    /// Create a new event query service.
    pub fn new(event_store: Arc<dyn EventStore>, snapshot_store: Arc<dyn SnapshotStore>) -> Self {
        Self {
            event_book_repo: Arc::new(EventBookRepository::new(
                event_store.clone(),
                snapshot_store,
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

    async fn get_events(
        &self,
        request: Request<Query>,
    ) -> Result<Response<Self::GetEventsStream>, Status> {
        let query = request.into_inner();
        let domain = query.domain;
        let root = query
            .root
            .ok_or_else(|| Status::invalid_argument("Query must have a root UUID"))?;

        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {}", e)))?;

        let event_book_repo = self.event_book_repo.clone();

        let (tx, rx) = tokio::sync::mpsc::channel(1);

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
                        let domain = query.domain.clone();
                        let root = match query.root {
                            Some(ref r) => match uuid::Uuid::from_slice(&r.value) {
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

                        // Support range queries if bounds are specified
                        let result = if query.lower_bound > 0 || query.upper_bound > 0 {
                            let upper = if query.upper_bound == 0 {
                                u32::MAX
                            } else {
                                query.upper_bound
                            };
                            event_book_repo
                                .get_from_to(&domain, root, query.lower_bound, upper)
                                .await
                        } else {
                            event_book_repo.get(&domain, root).await
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
    use crate::proto::{event_page, EventPage};
    use crate::test_utils::{MockEventStore, MockSnapshotStore};
    use prost_types::Any;
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
    async fn test_get_events_empty_aggregate() {
        let (service, _, _) = create_default_test_service();
        let root = uuid::Uuid::new_v4();

        let query = Query {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            lower_bound: 0,
            upper_bound: 0,
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
            synchronous: false,
        }];
        event_store.add("orders", root, events).await.unwrap();

        let query = Query {
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            lower_bound: 0,
            upper_bound: 0,
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
            domain: "orders".to_string(),
            root: None,
            lower_bound: 0,
            upper_bound: 0,
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
            domain: "orders".to_string(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // Invalid: must be 16 bytes
            }),
            lower_bound: 0,
            upper_bound: 0,
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
        event_store.add("orders", root1, vec![]).await.unwrap();
        event_store.add("orders", root2, vec![]).await.unwrap();

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
            .add("orders", uuid::Uuid::new_v4(), vec![])
            .await
            .unwrap();
        event_store
            .add("inventory", uuid::Uuid::new_v4(), vec![])
            .await
            .unwrap();

        let response = service.get_aggregate_roots(Request::new(())).await;

        assert!(response.is_ok());
        let stream = response.unwrap().into_inner();
        let roots: Vec<_> = stream.collect().await;
        assert_eq!(roots.len(), 2);
    }
}
