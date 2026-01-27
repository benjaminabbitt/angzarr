//! Aspect-oriented advice for cross-cutting concerns.
//!
//! This module provides wrapper types that add orthogonal behavior
//! (metrics, tracing, retries) without polluting core business logic.
//!
//! # Architecture
//!
//! Advice is applied at service composition time, not in implementations:
//!
//! ```ignore
//! // Core implementation - pure business logic
//! let store = SqliteEventStore::new(pool);
//!
//! // Apply advice layers
//! let store = Instrumented::new(store);
//!
//! // Use as normal - metrics are transparent
//! store.add(domain, root, events, correlation_id).await?;
//! ```
//!
//! # Available Advice
//!
//! - [`Instrumented`] - Adds metrics (counters, histograms) for all operations

mod instrumented;

pub use instrumented::Instrumented;
