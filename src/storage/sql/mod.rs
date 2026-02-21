//! Unified SQL storage implementations.
//!
//! This module provides shared implementations for SQL-based storage backends
//! (PostgreSQL, SQLite). The implementations are parameterized by database type
//! using the `SqlDatabase` trait.

mod position_store;
mod query;
mod snapshot_store;

pub use position_store::SqlPositionStore;
pub use query::SqlDatabase;
pub use snapshot_store::SqlSnapshotStore;

#[cfg(feature = "postgres")]
pub mod postgres {
    //! PostgreSQL database backend.

    use sea_query::PostgresQueryBuilder;
    use sqlx::PgPool;

    /// PostgreSQL database marker type.
    pub struct Postgres;

    impl super::SqlDatabase for Postgres {
        type Pool = PgPool;

        fn build_select(stmt: sea_query::SelectStatement) -> String {
            stmt.to_string(PostgresQueryBuilder)
        }

        fn build_insert(stmt: sea_query::InsertStatement) -> String {
            stmt.to_string(PostgresQueryBuilder)
        }

        fn build_delete(stmt: sea_query::DeleteStatement) -> String {
            stmt.to_string(PostgresQueryBuilder)
        }
    }

    /// PostgreSQL position store.
    pub type PostgresPositionStore = super::SqlPositionStore<Postgres>;

    /// PostgreSQL snapshot store.
    pub type PostgresSnapshotStore = super::SqlSnapshotStore<Postgres>;
}

#[cfg(feature = "sqlite")]
pub mod sqlite {
    //! SQLite database backend.

    use sea_query::SqliteQueryBuilder;
    use sqlx::SqlitePool;

    /// SQLite database marker type.
    pub struct Sqlite;

    impl super::SqlDatabase for Sqlite {
        type Pool = SqlitePool;

        fn build_select(stmt: sea_query::SelectStatement) -> String {
            stmt.to_string(SqliteQueryBuilder)
        }

        fn build_insert(stmt: sea_query::InsertStatement) -> String {
            stmt.to_string(SqliteQueryBuilder)
        }

        fn build_delete(stmt: sea_query::DeleteStatement) -> String {
            stmt.to_string(SqliteQueryBuilder)
        }
    }

    /// SQLite position store.
    pub type SqlitePositionStore = super::SqlPositionStore<Sqlite>;

    /// SQLite snapshot store.
    pub type SqliteSnapshotStore = super::SqlSnapshotStore<Sqlite>;
}
