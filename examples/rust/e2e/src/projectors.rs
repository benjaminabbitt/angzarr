//! SQLite projector utilities for standalone mode E2E tests.

use sqlx::SqlitePool;

/// Create shared in-memory SQLite pool for projector tests.
///
/// Uses a named in-memory database with shared cache so multiple
/// connections (projector writer + test reader) see the same data.
pub async fn create_projector_pool(name: &str) -> Result<SqlitePool, sqlx::Error> {
    use sqlx::sqlite::SqlitePoolOptions;

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:file:{}?mode=memory&cache=shared", name))
        .await
}
