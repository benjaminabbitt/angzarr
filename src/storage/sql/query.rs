//! SQL database abstraction trait.

/// Trait for SQL database backends.
///
/// This trait abstracts over different SQL databases (PostgreSQL, SQLite)
/// by providing the pool type and query building method.
pub trait SqlDatabase: Send + Sync + 'static {
    /// The connection pool type for this database.
    type Pool: Clone + Send + Sync;

    /// Build a SQL query string from a sea-query SELECT statement.
    fn build_select(stmt: sea_query::SelectStatement) -> String;

    /// Build a SQL query string from a sea-query INSERT statement.
    fn build_insert(stmt: sea_query::InsertStatement) -> String;

    /// Build a SQL query string from a sea-query DELETE statement.
    fn build_delete(stmt: sea_query::DeleteStatement) -> String;
}
