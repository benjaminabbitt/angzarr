//! Interface tests for storage and messaging backends.
//!
//! These tests verify trait contracts across all implementations using Gherkin/Cucumber.
//! Backend selection is controlled via environment variables:
//!
//! - `STORAGE_BACKEND`: sqlite (default), postgres, redis, immudb
//! - `BUS_BACKEND`: channel (default), ipc, amqp, kafka, sns_sqs, pubsub
//!
//! Run specific backend tests:
//!
//! ```bash
//! # SQLite (default)
//! cargo test --test interfaces --features sqlite
//!
//! # PostgreSQL
//! STORAGE_BACKEND=postgres cargo test --test interfaces --features postgres
//!
//! # Redis
//! STORAGE_BACKEND=redis cargo test --test interfaces --features redis
//! ```

pub mod backend;
pub mod steps;
