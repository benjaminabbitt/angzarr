//! Acceptance test framework for angzarr deployed services.
//!
//! This crate provides a cucumber-rs based test runner that connects to deployed
//! angzarr services via gRPC. It is used to validate ALL language implementations
//! (Rust, Go, Python, Java, C#, C++) when deployed to Kind/k3s/k8s clusters.
//!
//! # Usage
//!
//! Run against deployed services:
//! ```bash
//! # Port-forward services first
//! kubectl port-forward -n angzarr svc/player-aggregate 1310:1310 &
//! kubectl port-forward -n angzarr svc/table-aggregate 1311:1310 &
//! kubectl port-forward -n angzarr svc/hand-aggregate 1312:1310 &
//!
//! # Run acceptance tests
//! cargo test --package acceptance-tests
//! ```
//!
//! # Environment Variables
//!
//! - `PLAYER_ENDPOINT`: Player aggregate coordinator (default: `http://localhost:1310`)
//! - `TABLE_ENDPOINT`: Table aggregate coordinator (default: `http://localhost:1311`)
//! - `HAND_ENDPOINT`: Hand aggregate coordinator (default: `http://localhost:1312`)

pub mod steps;
pub mod world;

pub use world::AcceptanceWorld;
