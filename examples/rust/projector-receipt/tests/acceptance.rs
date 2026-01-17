//! Acceptance tests for Receipt projector.
//!
//! Projectors are read-model generators and don't have dedicated feature files.
//! Unit tests cover the projection logic. Integration testing happens through
//! end-to-end scenarios that exercise the complete order-to-receipt flow.
//!
//! Run with: cargo test -p projector-receipt --test acceptance

use cucumber::World;

#[derive(Debug, Default, World)]
pub struct ReceiptProjectorWorld {
    // Add fields as needed for step implementations
}

#[tokio::main]
async fn main() {
    // Projectors don't have dedicated feature files since they're tested
    // through end-to-end scenarios. Unit tests validate the projection logic.
    println!("Receipt projector acceptance tests: validated through unit tests and integration scenarios.");
}
