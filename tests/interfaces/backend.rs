//! Backend factory for interface tests.
//!
//! Provides a unified interface to create storage backends based on environment configuration.

use std::env;
use std::sync::Arc;

#[cfg(feature = "sqlite")]
use angzarr::storage::sqlite::{SqliteEventStore, SqlitePositionStore, SqliteSnapshotStore};

#[cfg(feature = "postgres")]
use angzarr::storage::postgres::{
    PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore,
};

#[cfg(feature = "redis")]
use angzarr::storage::redis::{RedisEventStore, RedisPositionStore, RedisSnapshotStore};

#[cfg(feature = "immudb")]
use angzarr::storage::immudb::{ImmudbEventStore, ImmudbPositionStore, ImmudbSnapshotStore};

use angzarr::storage::{EventStore, PositionStore, SnapshotStore};

#[cfg(feature = "postgres")]
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

#[cfg(feature = "redis")]
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

#[cfg(feature = "immudb")]
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

/// Storage backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    Sqlite,
    Postgres,
    Redis,
    Immudb,
}

impl StorageBackend {
    pub fn from_env() -> Self {
        match env::var("STORAGE_BACKEND")
            .unwrap_or_else(|_| "sqlite".to_string())
            .to_lowercase()
            .as_str()
        {
            "postgres" => StorageBackend::Postgres,
            "redis" => StorageBackend::Redis,
            "immudb" => StorageBackend::Immudb,
            _ => StorageBackend::Sqlite,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            StorageBackend::Sqlite => "sqlite",
            StorageBackend::Postgres => "postgres",
            StorageBackend::Redis => "redis",
            StorageBackend::Immudb => "immudb",
        }
    }
}

/// Container handles to keep containers alive during tests.
#[allow(dead_code)]
#[derive(Debug)]
pub enum ContainerHandle {
    None,
    #[cfg(feature = "postgres")]
    Postgres(testcontainers::ContainerAsync<GenericImage>),
    #[cfg(feature = "redis")]
    Redis(testcontainers::ContainerAsync<GenericImage>),
    #[cfg(feature = "immudb")]
    Immudb(testcontainers::ContainerAsync<GenericImage>),
}

/// Holds the storage implementations for a backend.
pub struct StorageContext {
    pub event_store: Arc<dyn EventStore>,
    pub snapshot_store: Arc<dyn SnapshotStore>,
    pub position_store: Arc<dyn PositionStore>,
    /// Container handle to keep container alive.
    #[allow(dead_code)]
    container: ContainerHandle,
}

impl std::fmt::Debug for StorageContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageContext")
            .field("event_store", &"<dyn EventStore>")
            .field("snapshot_store", &"<dyn SnapshotStore>")
            .field("position_store", &"<dyn PositionStore>")
            .field("container", &self.container)
            .finish()
    }
}

impl StorageContext {
    /// Create a storage context for the configured backend.
    pub async fn new(backend: StorageBackend) -> Self {
        match backend {
            StorageBackend::Sqlite => Self::create_sqlite().await,
            StorageBackend::Postgres => Self::create_postgres().await,
            StorageBackend::Redis => Self::create_redis().await,
            StorageBackend::Immudb => Self::create_immudb().await,
        }
    }

    #[cfg(feature = "sqlite")]
    async fn create_sqlite() -> Self {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create SQLite pool");

        // Run migrations
        sqlx::migrate!("./migrations/sqlite")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        StorageContext {
            event_store: Arc::new(SqliteEventStore::new(pool.clone())),
            snapshot_store: Arc::new(SqliteSnapshotStore::new(pool.clone())),
            position_store: Arc::new(SqlitePositionStore::new(pool)),
            container: ContainerHandle::None,
        }
    }

    #[cfg(not(feature = "sqlite"))]
    async fn create_sqlite() -> Self {
        panic!("SQLite feature not enabled. Build with --features sqlite");
    }

    #[cfg(feature = "postgres")]
    async fn create_postgres() -> Self {
        use sqlx::postgres::PgPoolOptions;

        let image = GenericImage::new("postgres", "16")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stdout(
                "database system is ready to accept connections",
            ));

        let container = image
            .with_env_var("POSTGRES_USER", "testuser")
            .with_env_var("POSTGRES_PASSWORD", "testpass")
            .with_env_var("POSTGRES_DB", "testdb")
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start Postgres container");

        // Brief delay for full readiness
        tokio::time::sleep(Duration::from_secs(2)).await;

        let host_port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");

        let connection_string =
            format!("postgres://testuser:testpass@{}:{}/testdb", host, host_port);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&connection_string)
            .await
            .expect("Failed to connect to PostgreSQL");

        // Run migrations
        sqlx::migrate!("./migrations/postgres")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        StorageContext {
            event_store: Arc::new(PostgresEventStore::new(pool.clone())),
            snapshot_store: Arc::new(PostgresSnapshotStore::new(pool.clone())),
            position_store: Arc::new(PostgresPositionStore::new(pool)),
            container: ContainerHandle::Postgres(container),
        }
    }

    #[cfg(not(feature = "postgres"))]
    async fn create_postgres() -> Self {
        panic!("PostgreSQL feature not enabled. Build with --features postgres");
    }

    #[cfg(feature = "redis")]
    async fn create_redis() -> Self {
        let image = GenericImage::new("redis", "7")
            .with_exposed_port(6379.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"));

        let container = image
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start Redis container");

        tokio::time::sleep(Duration::from_secs(1)).await;

        let host_port = container
            .get_host_port_ipv4(6379)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");

        let redis_url = format!("redis://{}:{}", host, host_port);

        StorageContext {
            event_store: Arc::new(
                RedisEventStore::new(&redis_url, None)
                    .await
                    .expect("Failed to create RedisEventStore"),
            ),
            snapshot_store: Arc::new(
                RedisSnapshotStore::new(&redis_url, None)
                    .await
                    .expect("Failed to create RedisSnapshotStore"),
            ),
            position_store: Arc::new(
                RedisPositionStore::new(&redis_url, None)
                    .await
                    .expect("Failed to create RedisPositionStore"),
            ),
            container: ContainerHandle::Redis(container),
        }
    }

    #[cfg(not(feature = "redis"))]
    async fn create_redis() -> Self {
        panic!("Redis feature not enabled. Build with --features redis");
    }

    #[cfg(feature = "immudb")]
    async fn create_immudb() -> Self {
        use angzarr::storage::immudb::ImmudbConfig;

        let image = GenericImage::new("codenotary/immudb", "1.9")
            .with_exposed_port(3322.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Web API server enabled"));

        let container = image
            .with_env_var("IMMUDB_ADDRESS", "0.0.0.0")
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start immudb container");

        tokio::time::sleep(Duration::from_secs(2)).await;

        let host_port = container
            .get_host_port_ipv4(3322)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");

        let config = ImmudbConfig {
            host: host.to_string(),
            port: host_port,
            username: "immudb".to_string(),
            password: "immudb".to_string(),
            database: "defaultdb".to_string(),
        };

        let event_store = ImmudbEventStore::new(config.clone())
            .await
            .expect("Failed to create event store");

        let snapshot_store = ImmudbSnapshotStore::new(config.clone())
            .await
            .expect("Failed to create snapshot store");

        let position_store = ImmudbPositionStore::new(config)
            .await
            .expect("Failed to create position store");

        StorageContext {
            event_store: Arc::new(event_store),
            snapshot_store: Arc::new(snapshot_store),
            position_store: Arc::new(position_store),
            container: ContainerHandle::Immudb(container),
        }
    }

    #[cfg(not(feature = "immudb"))]
    async fn create_immudb() -> Self {
        panic!("immudb feature not enabled. Build with --features immudb");
    }
}
