//! Backend abstraction for E2E tests.
//!
//! Provides a unified interface for command execution and event queries
//! across standalone (in-process) and gateway (gRPC) test modes.
//!
//! Backend selection via `ANGZARR_TEST_MODE` env var:
//! - `standalone` (default): In-process runtime with SQLite memory storage
//! - `gateway`: Remote gRPC gateway at `ANGZARR_ENDPOINT`

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use angzarr::client_traits::SpeculativeClient as SpeculativeClientTrait;
use angzarr::orchestration::aggregate::DEFAULT_EDITION;
use angzarr::proto::{CommandBook, CommandResponse, EventPage};
use angzarr::standalone::DomainStorage;

pub type BackendResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Unified backend for E2E test command execution and event queries.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Execute a command and return the response.
    async fn execute(&self, command: CommandBook) -> BackendResult<CommandResponse>;

    /// Clean up test data (called before test run).
    /// Default implementation does nothing (for in-memory backends).
    async fn cleanup(&self) -> BackendResult<()> {
        Ok(())
    }

    /// Query all events for a domain/root.
    async fn query_events(&self, domain: &str, root: Uuid) -> BackendResult<Vec<EventPage>>;

    /// Query events for a specific edition.
    async fn query_events_in_edition(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> BackendResult<Vec<EventPage>>;

    /// Query events at a temporal point (by sequence or timestamp).
    async fn query_events_temporal(
        &self,
        domain: &str,
        root: Uuid,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<Vec<EventPage>>;

    /// Speculatively execute a command against temporal state.
    ///
    /// Returns the events that *would* be produced without persisting,
    /// publishing, or triggering any side effects (sagas, projectors, PMs).
    async fn dry_run(
        &self,
        command: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<CommandResponse>;

    /// Query all events across domains for a given correlation ID.
    /// Returns (domain, event_type, root) tuples.
    async fn query_by_correlation(
        &self,
        correlation_id: &str,
    ) -> BackendResult<Vec<(String, String, Uuid)>> {
        let _ = correlation_id;
        Ok(vec![])
    }

    /// Delete all events for a specific aggregate (domain/root).
    /// Used for test setup to ensure fresh state.
    /// Default implementation does nothing (standalone uses in-memory storage).
    async fn delete_aggregate(&self, domain: &str, root: Uuid) -> BackendResult<()> {
        let _ = (domain, root);
        Ok(())
    }
}

/// Result of backend creation, including projector database pools.
pub struct BackendWithProjectors {
    pub backend: Arc<dyn Backend>,
    pub web_db: Option<sqlx::SqlitePool>,
    pub accounting_db: Option<sqlx::SqlitePool>,
    pub speculative: Option<Arc<dyn angzarr::client_traits::SpeculativeClient>>,
}

/// Create the appropriate backend based on `ANGZARR_TEST_MODE` env var.
pub async fn create_backend() -> BackendWithProjectors {
    let mode = std::env::var("ANGZARR_TEST_MODE").unwrap_or_else(|_| "standalone".into());
    match mode.as_str() {
        "gateway" => {
            let (backend, speculative) = create_gateway_backend().await;
            BackendWithProjectors {
                backend: Arc::new(backend),
                web_db: None,
                accounting_db: None,
                speculative: Some(Arc::new(speculative)),
            }
        }
        _ => create_standalone_with_projectors().await,
    }
}

// ============================================================================
// Standalone Backend
// ============================================================================

use angzarr::handlers::projectors::topology::TopologyProjector;
use angzarr::standalone::{
    CommandClient, ProcessManagerConfig, ProjectorConfig, Runtime, RuntimeBuilder, SagaConfig,
};
use angzarr::storage::SqliteTopologyStore;
use tokio::sync::OnceCell;

use crate::adapters::{AggregateLogicAdapter, SagaLogicAdapter};
use crate::mock_services::MockFraudServer;
use crate::projectors::create_projector_pool;

/// Shared mock fraud server across all test scenarios.
///
/// Configured with standard test responses:
/// - CUST-FRAUD -> declined
/// - CUST-REVIEW -> review_required
/// - Any other customer_id -> approved (default)
static SHARED_FRAUD_SERVER: OnceCell<Arc<MockFraudServer>> = OnceCell::const_new();

async fn shared_fraud_server() -> Arc<MockFraudServer> {
    SHARED_FRAUD_SERVER
        .get_or_init(|| async {
            let mut responses = std::collections::HashMap::new();
            responses.insert("CUST-FRAUD".to_string(), "declined".to_string());
            responses.insert("CUST-REVIEW".to_string(), "review_required".to_string());
            Arc::new(MockFraudServer::start_with_responses(responses).await)
        })
        .await
        .clone()
}

/// Shared topology projector across all test scenarios.
///
/// Intentionally not reset between tests — accumulates the full topology
/// graph from the entire test suite for a realistic Grafana view.
static SHARED_TOPOLOGY: OnceCell<Arc<TopologyProjector>> = OnceCell::const_new();

async fn shared_topology_projector() -> Arc<TopologyProjector> {
    SHARED_TOPOLOGY
        .get_or_init(|| async {
            let pool = create_projector_pool("e2e_topology")
                .await
                .expect("Failed to create topology pool");
            let store = Arc::new(SqliteTopologyStore::new(pool));
            let projector = Arc::new(TopologyProjector::new(store, 0));
            projector
                .init()
                .await
                .expect("Failed to init shared topology projector");
            projector
        })
        .await
        .clone()
}

/// In-process standalone backend using RuntimeBuilder.
struct StandaloneBackend {
    client: CommandClient,
    domain_stores: HashMap<String, DomainStorage>,
    // Runtime kept alive for event distribution (projectors, sagas, PMs)
    _runtime: Runtime,
}

async fn create_standalone_with_projectors() -> BackendWithProjectors {
    use agg_fulfillment::FulfillmentLogic;
    use agg_inventory::InventoryLogic;
    use agg_order::OrderLogic;
    use pmg_fulfillment::OrderFulfillmentProcess;
    use prj_inventory::InventoryProjector;
    use sag_order_fulfillment::OrderFulfillmentSaga;
    use sag_order_inventory::OrderInventorySaga;
    use sag_fulfillment_inventory::FulfillmentInventorySaga;

    // Shared mock fraud server for external service integration tests
    let fraud_server = shared_fraud_server().await;
    let fraud_url = fraud_server.url();

    // Shared topology projector — accumulates across all scenarios
    let topology_projector = shared_topology_projector().await;

    // Create OrderLogic with fraud service URL for external service integration
    let order_logic = OrderLogic::with_fraud_service_url(Some(&fraud_url));

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        // Topology visualization
        .register_topology(topology_projector, ProjectorConfig::async_())
        // 3 aggregate domains
        .register_aggregate("order", AggregateLogicAdapter::new(order_logic))
        .register_aggregate(
            "fulfillment",
            AggregateLogicAdapter::new(FulfillmentLogic::new()),
        )
        .register_aggregate(
            "inventory",
            AggregateLogicAdapter::new(InventoryLogic::new()),
        )
        // 3 sagas
        .register_saga(
            "order-fulfillment-saga",
            SagaLogicAdapter::new(OrderFulfillmentSaga::new()),
            SagaConfig::new("order", "fulfillment"),
        )
        .register_saga(
            "order-inventory-saga",
            SagaLogicAdapter::new(OrderInventorySaga::new()),
            SagaConfig::new("order", "inventory"),
        )
        .register_saga(
            "fulfillment-inventory-saga",
            SagaLogicAdapter::new(FulfillmentInventorySaga::new()),
            SagaConfig::new("fulfillment", "inventory"),
        )
        // 1 process manager
        .register_process_manager(
            "order-fulfillment",
            OrderFulfillmentProcess::new(),
            ProcessManagerConfig::new("order-fulfillment"),
        )
        // 1 projector
        .register_projector(
            "inventory",
            InventoryProjector::new(),
            ProjectorConfig::async_().with_domains(vec!["inventory".into()]),
        )
        .build()
        .await
        .expect("Failed to build standalone runtime");

    let client = runtime.command_client();
    let speculative = runtime.speculative_client();
    let domain_stores = runtime.domain_stores().clone();
    runtime.start().await.expect("Failed to start runtime");

    BackendWithProjectors {
        backend: Arc::new(StandaloneBackend {
            client,
            domain_stores,
            _runtime: runtime,
        }),
        web_db: None,
        accounting_db: None,
        speculative: Some(Arc::new(speculative)),
    }
}

#[async_trait]
impl Backend for StandaloneBackend {
    async fn execute(&self, command: CommandBook) -> BackendResult<CommandResponse> {
        self.client.execute(command).await
    }

    async fn query_events(&self, domain: &str, root: Uuid) -> BackendResult<Vec<EventPage>> {
        let storage = self
            .domain_stores
            .get(domain)
            .ok_or_else(|| format!("No storage for domain: {}", domain))?;
        let pages = storage.event_store.get(domain, DEFAULT_EDITION, root).await?;
        Ok(pages)
    }

    async fn query_events_in_edition(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> BackendResult<Vec<EventPage>> {
        let storage = self
            .domain_stores
            .get(domain)
            .ok_or_else(|| format!("No storage for domain: {}", domain))?;
        let pages = storage.event_store.get(domain, edition, root).await?;
        Ok(pages)
    }

    async fn query_events_temporal(
        &self,
        domain: &str,
        root: Uuid,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<Vec<EventPage>> {
        let storage = self
            .domain_stores
            .get(domain)
            .ok_or_else(|| format!("No storage for domain: {}", domain))?;
        if let Some(seq) = as_of_sequence {
            let pages = storage
                .event_store
                .get_from_to(domain, DEFAULT_EDITION, root, 0, seq + 1)
                .await?;
            Ok(pages)
        } else if let Some(ts) = as_of_timestamp {
            let pages = storage
                .event_store
                .get_until_timestamp(domain, DEFAULT_EDITION, root, ts)
                .await?;
            Ok(pages)
        } else {
            Err("query_events_temporal requires as_of_sequence or as_of_timestamp".into())
        }
    }

    async fn dry_run(
        &self,
        command: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<CommandResponse> {
        self.client
            .dry_run(command, as_of_sequence, as_of_timestamp)
            .await
    }

    async fn query_by_correlation(
        &self,
        correlation_id: &str,
    ) -> BackendResult<Vec<(String, String, Uuid)>> {
        let mut results = Vec::new();
        for (domain, storage) in &self.domain_stores {
            let books = storage
                .event_store
                .get_by_correlation(correlation_id)
                .await?;
            for book in books {
                let root = book
                    .cover
                    .as_ref()
                    .and_then(|c| c.root.as_ref())
                    .and_then(|r| Uuid::from_slice(&r.value).ok())
                    .unwrap_or_default();
                for page in &book.pages {
                    if let Some(event) = &page.event {
                        let event_type = event
                            .type_url
                            .rsplit('/')
                            .next()
                            .unwrap_or(&event.type_url)
                            .to_string();
                        results.push((domain.clone(), event_type, root));
                    }
                }
            }
        }
        Ok(results)
    }
}

// ============================================================================
// Gateway Backend
// ============================================================================

use angzarr::proto::{temporal_query::PointInTime, DryRunRequest, TemporalQuery};
use angzarr_client::traits::GatewayClient as GatewayClientTrait;
use angzarr_client::{parse_timestamp, Client, QueryBuilderExt};

/// Remote gRPC gateway backend using angzarr-client.
struct GatewayBackend {
    client: Client,
    #[cfg(feature = "gateway-cleanup")]
    mongodb_uri: String,
}

async fn create_gateway_backend() -> (GatewayBackend, angzarr_client::SpeculativeClient) {
    let client = Client::from_env("ANGZARR_ENDPOINT", "http://localhost:50051")
        .await
        .expect("Failed to connect to gateway");

    let speculative = client.speculative.clone();

    let backend = GatewayBackend {
        client,
        #[cfg(feature = "gateway-cleanup")]
        mongodb_uri: std::env::var("ANGZARR_MONGODB_URI")
            .unwrap_or_else(|_| "mongodb://angzarr:angzarr-dev@localhost:27017/angzarr?authSource=angzarr".into()),
    };

    // Clean up before tests run
    backend.cleanup().await.expect("Failed to cleanup before tests");

    (backend, speculative)
}

#[async_trait]
impl Backend for GatewayBackend {
    async fn execute(&self, command: CommandBook) -> BackendResult<CommandResponse> {
        let response = self.client.gateway.execute(command).await?;
        Ok(response)
    }

    #[cfg(feature = "gateway-cleanup")]
    async fn cleanup(&self) -> BackendResult<()> {
        use tracing::{info, warn};

        info!("Cleaning up MongoDB before tests...");

        // Try to connect with a short timeout
        let options = mongodb::options::ClientOptions::parse(&self.mongodb_uri)
            .await
            .map_err(|e| format!("Failed to parse MongoDB URI: {}", e))?;

        let client = match mongodb::Client::with_options(options) {
            Ok(c) => c,
            Err(e) => {
                warn!("Could not connect to MongoDB for cleanup: {}. Tests may fail if data exists from previous runs.", e);
                warn!("To enable cleanup, port-forward MongoDB: kubectl port-forward -n angzarr svc/angzarr-db-mongodb 27017:27017");
                return Ok(());
            }
        };

        let db = client.database("angzarr");

        // Test connection before cleanup
        if let Err(e) = db.list_collection_names().await {
            warn!("MongoDB not accessible for cleanup: {}. Tests may fail if data exists from previous runs.", e);
            warn!("To enable cleanup, port-forward MongoDB: kubectl port-forward -n angzarr svc/angzarr-db-mongodb 27017:27017");
            return Ok(());
        }

        // Drop events and snapshots collections
        if let Err(e) = db
            .collection::<mongodb::bson::Document>("events")
            .drop()
            .await
        {
            warn!("Failed to drop events collection: {}", e);
        }
        if let Err(e) = db
            .collection::<mongodb::bson::Document>("snapshots")
            .drop()
            .await
        {
            warn!("Failed to drop snapshots collection: {}", e);
        }

        info!("MongoDB cleanup complete");
        Ok(())
    }

    #[cfg(feature = "gateway-cleanup")]
    async fn delete_aggregate(&self, domain: &str, root: Uuid) -> BackendResult<()> {
        use mongodb::bson::doc;
        use tracing::debug;

        let options = mongodb::options::ClientOptions::parse(&self.mongodb_uri)
            .await
            .map_err(|e| format!("Failed to parse MongoDB URI: {}", e))?;

        let client = mongodb::Client::with_options(options)
            .map_err(|e| format!("Failed to connect to MongoDB: {}", e))?;

        let db = client.database("angzarr");
        let root_bytes = root.as_bytes().to_vec();

        // Delete events for this domain/root
        let events_result = db
            .collection::<mongodb::bson::Document>("events")
            .delete_many(doc! {
                "domain": domain,
                "root": mongodb::bson::Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: root_bytes.clone() }
            })
            .await;

        if let Ok(result) = events_result {
            debug!(domain, %root, deleted = result.deleted_count, "Deleted events for aggregate");
        }

        // Delete snapshots for this domain/root
        let _ = db
            .collection::<mongodb::bson::Document>("snapshots")
            .delete_many(doc! {
                "domain": domain,
                "root": mongodb::bson::Binary { subtype: mongodb::bson::spec::BinarySubtype::Generic, bytes: root_bytes }
            })
            .await;

        Ok(())
    }

    async fn query_events(&self, domain: &str, root: Uuid) -> BackendResult<Vec<EventPage>> {
        let event_book = self
            .client
            .query
            .query(domain, root)
            .range(0)
            .get_event_book()
            .await?;
        Ok(event_book.pages)
    }

    async fn query_events_in_edition(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
    ) -> BackendResult<Vec<EventPage>> {
        let event_book = self
            .client
            .query
            .query(domain, root)
            .edition(edition)
            .range(0)
            .get_event_book()
            .await?;
        Ok(event_book.pages)
    }

    async fn query_events_temporal(
        &self,
        domain: &str,
        root: Uuid,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<Vec<EventPage>> {
        if let Some(seq) = as_of_sequence {
            let event_book = self
                .client
                .query
                .query(domain, root)
                .as_of_sequence(seq)
                .get_event_book()
                .await?;
            Ok(event_book.pages)
        } else if let Some(ts) = as_of_timestamp {
            let event_book = self
                .client
                .query
                .query(domain, root)
                .as_of_time(ts)?
                .get_event_book()
                .await?;
            Ok(event_book.pages)
        } else {
            Err("query_events_temporal requires as_of_sequence or as_of_timestamp".into())
        }
    }

    async fn dry_run(
        &self,
        command: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<CommandResponse> {
        let point_in_time = if let Some(seq) = as_of_sequence {
            Some(TemporalQuery {
                point_in_time: Some(PointInTime::AsOfSequence(seq)),
            })
        } else {
            as_of_timestamp
                .map(|ts| {
                    Ok::<_, angzarr_client::ClientError>(TemporalQuery {
                        point_in_time: Some(PointInTime::AsOfTime(parse_timestamp(ts)?)),
                    })
                })
                .transpose()?
        };

        let request = DryRunRequest {
            command: Some(command),
            point_in_time,
        };

        let response = self.client.speculative.dry_run(request).await?;
        Ok(response)
    }

    async fn query_by_correlation(
        &self,
        correlation_id: &str,
    ) -> BackendResult<Vec<(String, String, Uuid)>> {
        const DOMAINS: &[&str] = &["order", "fulfillment", "inventory"];

        let mut results = Vec::new();

        for domain in DOMAINS {
            let books = self
                .client
                .query
                .query_domain(*domain)
                .by_correlation_id(correlation_id)
                .get_events()
                .await;

            match books {
                Ok(books) => {
                    for book in books {
                        let root = book
                            .cover
                            .as_ref()
                            .and_then(|c| c.root.as_ref())
                            .and_then(|r| Uuid::from_slice(&r.value).ok())
                            .unwrap_or_default();
                        let book_domain = book
                            .cover
                            .as_ref()
                            .map(|c| c.domain.clone())
                            .unwrap_or_else(|| domain.to_string());
                        for page in &book.pages {
                            if let Some(event) = &page.event {
                                let event_type = event
                                    .type_url
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or(&event.type_url)
                                    .to_string();
                                results.push((book_domain.clone(), event_type, root));
                            }
                        }
                    }
                }
                Err(_) => {
                    // Domain may not have events for this correlation; continue
                }
            }
        }

        Ok(results)
    }
}
