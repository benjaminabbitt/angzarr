//! Angzarr - CQRS/ES Framework
//!
//! A Rust implementation of the angzarr framework for building
//! event-sourced applications with CQRS architecture.

pub mod bus;
pub mod clients;
pub mod config;
pub mod discovery;
pub mod grpc;
pub mod handlers;
pub mod process;
pub mod repository;
pub mod services;
#[cfg(feature = "sqlite")]
pub mod standalone;
pub mod storage;
pub mod transport;
pub mod utils;

pub mod proto {
    tonic::include_proto!("angzarr");
}
