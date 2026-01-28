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

use angzarr::proto::{CommandBook, CommandResponse, EventPage};
use angzarr::standalone::DomainStorage;

pub type BackendResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Unified backend for E2E test command execution and event queries.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Execute a command and return the response.
    async fn execute(&self, command: CommandBook) -> BackendResult<CommandResponse>;

    /// Query all events for a domain/root.
    async fn query_events(&self, domain: &str, root: Uuid) -> BackendResult<Vec<EventPage>>;

    /// Query events at a temporal point (by sequence or timestamp).
    async fn query_events_temporal(
        &self,
        domain: &str,
        root: Uuid,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) -> BackendResult<Vec<EventPage>>;

    /// Dry-run a command against temporal state (no persistence).
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
}

/// Result of backend creation, including projector database pools.
pub struct BackendWithProjectors {
    pub backend: Arc<dyn Backend>,
    pub web_db: Option<sqlx::SqlitePool>,
    pub accounting_db: Option<sqlx::SqlitePool>,
}

/// Create the appropriate backend based on `ANGZARR_TEST_MODE` env var.
pub async fn create_backend() -> BackendWithProjectors {
    let mode = std::env::var("ANGZARR_TEST_MODE").unwrap_or_else(|_| "standalone".into());
    match mode.as_str() {
        "gateway" => BackendWithProjectors {
            backend: Arc::new(create_gateway_backend().await),
            web_db: None,
            accounting_db: None,
        },
        _ => create_standalone_with_projectors().await,
    }
}

// ============================================================================
// Standalone Backend
// ============================================================================

use angzarr::standalone::{
    CommandClient, ProcessManagerConfig, ProjectorConfig, Runtime, RuntimeBuilder, SagaConfig,
};

use crate::adapters::{AggregateLogicAdapter, SagaLogicAdapter};
use crate::projectors::{create_projector_pool, AccountingProjector, WebProjector};

/// In-process standalone backend using RuntimeBuilder.
struct StandaloneBackend {
    client: CommandClient,
    domain_stores: HashMap<String, DomainStorage>,
    // Runtime kept alive for event distribution (projectors, sagas, PMs)
    _runtime: Runtime,
}

async fn create_standalone_with_projectors() -> BackendWithProjectors {
    use cart::CartLogic;
    use customer::CustomerLogic;
    use fulfillment::FulfillmentLogic;
    use inventory_svc::InventoryLogic;
    use order::OrderLogic;
    use process_manager_fulfillment::OrderFulfillmentProcess;
    use product::ProductLogic;
    use saga_cancellation::CancellationSaga;
    use saga_fulfillment::FulfillmentSaga;
    use saga_loyalty_earn::LoyaltyEarnSaga;

    // Create projector SQLite pools
    let web_pool = create_projector_pool("e2e_web_proj")
        .await
        .expect("Failed to create web projector pool");
    let accounting_pool = create_projector_pool("e2e_acct_proj")
        .await
        .expect("Failed to create accounting projector pool");

    // Create projector handlers
    let web_projector = WebProjector::new(web_pool.clone())
        .await
        .expect("Failed to init web projector");
    let accounting_projector = AccountingProjector::new(accounting_pool.clone())
        .await
        .expect("Failed to init accounting projector");

    let mut runtime = RuntimeBuilder::new()
        .with_sqlite_memory()
        // 6 aggregate domains
        .register_aggregate("cart", AggregateLogicAdapter::new(CartLogic::new()))
        .register_aggregate("customer", AggregateLogicAdapter::new(CustomerLogic::new()))
        .register_aggregate("order", AggregateLogicAdapter::new(OrderLogic::new()))
        .register_aggregate(
            "fulfillment",
            AggregateLogicAdapter::new(FulfillmentLogic::new()),
        )
        .register_aggregate(
            "inventory",
            AggregateLogicAdapter::new(InventoryLogic::new()),
        )
        .register_aggregate("product", AggregateLogicAdapter::new(ProductLogic::new()))
        // 3 sagas
        .register_saga(
            "fulfillment-saga",
            SagaLogicAdapter::new(FulfillmentSaga::new()),
            SagaConfig::new("order", "fulfillment"),
        )
        .register_saga(
            "cancellation-saga",
            SagaLogicAdapter::new(CancellationSaga::new()),
            SagaConfig::new("order", "inventory").with_output("customer"),
        )
        .register_saga(
            "loyalty-earn-saga",
            SagaLogicAdapter::new(LoyaltyEarnSaga::new()),
            SagaConfig::new("order", "customer"),
        )
        // 1 process manager
        .register_process_manager(
            "order-fulfillment",
            OrderFulfillmentProcess::new(),
            ProcessManagerConfig::new("order-fulfillment"),
        )
        // 2 projectors
        .register_projector(
            "web",
            web_projector,
            ProjectorConfig::async_().with_domains(vec!["order".into()]),
        )
        .register_projector(
            "accounting",
            accounting_projector,
            ProjectorConfig::async_().with_domains(vec!["order".into(), "customer".into()]),
        )
        .build()
        .await
        .expect("Failed to build standalone runtime");

    let client = runtime.command_client();
    let domain_stores = runtime.domain_stores().clone();
    runtime.start().await.expect("Failed to start runtime");

    BackendWithProjectors {
        backend: Arc::new(StandaloneBackend {
            client,
            domain_stores,
            _runtime: runtime,
        }),
        web_db: Some(web_pool),
        accounting_db: Some(accounting_pool),
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
        let pages = storage.event_store.get(domain, root).await?;
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
                .get_from_to(domain, root, 0, seq + 1)
                .await?;
            Ok(pages)
        } else if let Some(ts) = as_of_timestamp {
            let pages = storage
                .event_store
                .get_until_timestamp(domain, root, ts)
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

use angzarr::proto::{
    temporal_query::PointInTime, DryRunRequest, TemporalQuery,
};
use angzarr_client::{parse_timestamp, Client, QueryBuilderExt};
use angzarr_client::traits::GatewayClient as GatewayClientTrait;

/// Remote gRPC gateway backend using angzarr-client.
struct GatewayBackend {
    client: Client,
}

async fn create_gateway_backend() -> GatewayBackend {
    let client = Client::from_env("ANGZARR_ENDPOINT", "http://localhost:50051")
        .await
        .expect("Failed to connect to gateway");

    GatewayBackend { client }
}

#[async_trait]
impl Backend for GatewayBackend {
    async fn execute(&self, command: CommandBook) -> BackendResult<CommandResponse> {
        let response = self.client.gateway.execute(command).await?;
        Ok(response)
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

        let response = self.client.gateway.dry_run(request).await?;
        Ok(response)
    }
}
