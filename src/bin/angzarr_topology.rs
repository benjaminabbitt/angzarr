//! angzarr-topology: Topology visualization service
//!
//! Subscribes to all events on the message bus and builds a topology graph
//! of runtime components. Serves the graph via REST for Grafana's Node Graph
//! panel using the Node Graph API plugin.
//!
//! ## Architecture
//! ```text
//! [Event Bus] -> [angzarr-topology] -> [TopologyProjector]
//!                        |                      |
//!                        v                      v
//!                 [REST API :9099]       [Storage Backend]
//!                        |
//!                        v
//!                 [Grafana Node Graph]
//! ```
//!
//! ## Configuration
//! - TOPOLOGY_REST_PORT: REST API port (default: 9099)
//! - TOPOLOGY_STORAGE_TYPE: "sqlite", "postgres", "mongodb", or "redis" (default: sqlite)
//! - TOPOLOGY_SQLITE_PATH: SQLite database path (default: /data/topology.db)
//! - TOPOLOGY_POSTGRES_URI: PostgreSQL connection URI
//! - TOPOLOGY_MONGODB_URI: MongoDB connection URI (default: mongodb://localhost:27017)
//! - TOPOLOGY_MONGODB_DATABASE: MongoDB database name (default: angzarr)
//! - TOPOLOGY_REDIS_URI: Redis connection URI (default: redis://localhost:6379)
//! - MESSAGING_TYPE: amqp, kafka, or channel

use std::sync::Arc;

use futures::future::BoxFuture;
use tracing::{error, info};

use angzarr::bus::{init_event_bus, BusError, EventBusMode, EventHandler};
#[cfg(feature = "mongodb")]
use angzarr::config::{TOPOLOGY_MONGODB_DATABASE_ENV_VAR, TOPOLOGY_MONGODB_URI_ENV_VAR};
#[cfg(feature = "postgres")]
use angzarr::config::TOPOLOGY_POSTGRES_URI_ENV_VAR;
#[cfg(feature = "redis")]
use angzarr::config::TOPOLOGY_REDIS_URI_ENV_VAR;
#[cfg(feature = "sqlite")]
use angzarr::config::TOPOLOGY_SQLITE_PATH_ENV_VAR;
use angzarr::config::{Config, TOPOLOGY_REST_PORT_ENV_VAR, TOPOLOGY_STORAGE_TYPE_ENV_VAR};
use angzarr::handlers::projectors::topology::store::TopologyStore;
use angzarr::handlers::projectors::topology::TopologyProjector;
use angzarr::proto::EventBook;
use angzarr::utils::bootstrap::init_tracing;

/// Bridges bus events to the topology projector.
struct TopologyEventHandler {
    projector: Arc<TopologyProjector>,
}

impl EventHandler for TopologyEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let projector = Arc::clone(&self.projector);
        Box::pin(async move {
            if let Err(e) = projector.process_event(&book).await {
                error!(error = %e, "topology projector handle failed");
            }
            Ok(())
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let rest_port: u16 = std::env::var(TOPOLOGY_REST_PORT_ENV_VAR)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9099);

    let storage_type =
        std::env::var(TOPOLOGY_STORAGE_TYPE_ENV_VAR).unwrap_or_else(|_| "sqlite".to_string());

    info!(port = rest_port, storage = %storage_type, "starting angzarr-topology");

    let store: Arc<dyn TopologyStore> = match storage_type.as_str() {
        #[cfg(feature = "sqlite")]
        "sqlite" => {
            use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
            use std::time::Duration;

            let path = std::env::var(TOPOLOGY_SQLITE_PATH_ENV_VAR)
                .unwrap_or_else(|_| "/data/topology.db".to_string());

            let opts = SqliteConnectOptions::new()
                .filename(&path)
                .journal_mode(SqliteJournalMode::Wal)
                .busy_timeout(Duration::from_secs(30))
                .create_if_missing(true);

            let pool = SqlitePoolOptions::new()
                .max_connections(5)
                .connect_with(opts)
                .await?;

            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&pool)
                .await?;

            Arc::new(angzarr::storage::sqlite::SqliteTopologyStore::new(pool))
        }
        #[cfg(feature = "postgres")]
        "postgres" => {
            let uri = std::env::var(TOPOLOGY_POSTGRES_URI_ENV_VAR)
                .unwrap_or_else(|_| "postgres://localhost:5432/angzarr".to_string());

            let pool = sqlx::PgPool::connect(&uri).await?;
            Arc::new(angzarr::storage::postgres::PostgresTopologyStore::new(pool))
        }
        #[cfg(feature = "mongodb")]
        "mongodb" => {
            let uri = std::env::var(TOPOLOGY_MONGODB_URI_ENV_VAR)
                .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
            let database = std::env::var(TOPOLOGY_MONGODB_DATABASE_ENV_VAR)
                .unwrap_or_else(|_| "angzarr".to_string());

            let client = mongodb::Client::with_uri_str(&uri).await?;
            Arc::new(
                angzarr::storage::mongodb::MongoTopologyStore::new(&client, &database)
                    .await
                    .map_err(|e| format!("failed to create MongoDB topology store: {}", e))?,
            )
        }
        #[cfg(feature = "redis")]
        "redis" => {
            let uri = std::env::var(TOPOLOGY_REDIS_URI_ENV_VAR)
                .unwrap_or_else(|_| "redis://localhost:6379".to_string());

            Arc::new(
                angzarr::storage::redis::RedisTopologyStore::new(&uri, Some("angzarr"))
                    .await
                    .map_err(|e| format!("failed to create Redis topology store: {}", e))?,
            )
        }
        _ => {
            return Err(format!("unsupported topology storage type: {}", storage_type).into());
        }
    };

    store
        .init_schema()
        .await
        .map_err(|e| -> Box<dyn std::error::Error> {
            format!("failed to init topology schema: {}", e).into()
        })?;

    // Start REST server
    let rest_store = Arc::clone(&store);
    tokio::spawn(async move {
        if let Err(e) =
            angzarr::handlers::projectors::topology::rest::serve(rest_store, rest_port).await
        {
            error!(error = %e, "topology REST server failed");
        }
    });

    let config_path = angzarr::utils::bootstrap::parse_config_path();
    let config = Config::load(config_path.as_deref()).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    let messaging = config
        .messaging
        .as_ref()
        .ok_or("Topology service requires 'messaging' configuration")?;

    info!(messaging_type = ?messaging.messaging_type, "connecting to event bus");

    let subscriber = init_event_bus(
        messaging,
        EventBusMode::SubscriberAll {
            queue: "topology".to_string(),
        },
    )
    .await
    .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    let projector = Arc::new(TopologyProjector::new(store, rest_port));

    let handler = TopologyEventHandler { projector };

    subscriber
        .subscribe(Box::new(handler))
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    subscriber
        .start_consuming()
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    info!("topology service running, press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    Ok(())
}
