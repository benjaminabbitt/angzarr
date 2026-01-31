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
//!                 [REST API :9099]       [SQLite/Postgres]
//!                        |
//!                        v
//!                 [Grafana Node Graph]
//! ```
//!
//! ## Configuration
//! - TOPOLOGY_REST_PORT: REST API port (default: 9099)
//! - TOPOLOGY_STORAGE_TYPE: "sqlite" or "postgres" (default: sqlite)
//! - TOPOLOGY_SQLITE_PATH: SQLite database path (default: /data/topology.db)
//! - MESSAGING_TYPE: amqp, kafka, or channel

use std::sync::Arc;

use futures::future::BoxFuture;
use tracing::{error, info};

use angzarr::bus::{init_event_bus, BusError, EventBusMode, EventHandler};
use angzarr::config::Config;
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

    let rest_port: u16 = std::env::var("TOPOLOGY_REST_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9099);

    let storage_type =
        std::env::var("TOPOLOGY_STORAGE_TYPE").unwrap_or_else(|_| "sqlite".to_string());

    info!(port = rest_port, storage = %storage_type, "starting angzarr-topology");

    let store: Arc<dyn TopologyStore> = match storage_type.as_str() {
        #[cfg(feature = "sqlite")]
        "sqlite" => {
            use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
            use std::time::Duration;

            let path = std::env::var("TOPOLOGY_SQLITE_PATH")
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
            let uri = std::env::var("TOPOLOGY_POSTGRES_URI")
                .unwrap_or_else(|_| "postgres://localhost:5432/angzarr".to_string());

            let pool = sqlx::PgPool::connect(&uri).await?;
            Arc::new(angzarr::storage::postgres::PostgresTopologyStore::new(pool))
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
