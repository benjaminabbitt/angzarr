//! Acceptance test runner for angzarr deployed services.
//!
//! This test runner connects to deployed angzarr services via gRPC and verifies
//! they work correctly. It is used to validate ALL language implementations
//! (Rust, Go, Python, Java, C#, C++) when deployed to Kind/k3s/k8s clusters.
//!
//! # Usage
//!
//! ```bash
//! # Port-forward services first
//! kubectl port-forward -n angzarr svc/player-aggregate 1310:1310 &
//! kubectl port-forward -n angzarr svc/table-aggregate 1311:1310 &
//! kubectl port-forward -n angzarr svc/hand-aggregate 1312:1310 &
//!
//! # Run acceptance tests
//! cargo test --package acceptance-tests --test acceptance
//!
//! # Run only @container tagged tests
//! cargo test --package acceptance-tests --test acceptance -- --tags @container
//! ```
//!
//! # Environment Variables
//!
//! - `PLAYER_ENDPOINT`: Player aggregate coordinator (default: `http://localhost:1310`)
//! - `TABLE_ENDPOINT`: Table aggregate coordinator (default: `http://localhost:1311`)
//! - `HAND_ENDPOINT`: Hand aggregate coordinator (default: `http://localhost:1312`)
//!
//! # NOTE: Cross-Language Validation
//!
//! This acceptance test framework validates deployed services regardless of
//! implementation language. The same tests run against:
//! - Rust examples (examples/rust/)
//! - Go examples (examples/go/)
//! - Python examples (examples/python/)
//! - Java examples (examples/java/)
//! - C# examples (examples/csharp/)
//! - C++ examples (examples/cpp/)
//!
//! Business logic is language-specific; the wire protocol (gRPC/protobuf) is shared.

use acceptance_tests::AcceptanceWorld;
use cucumber::World;

#[tokio::main]
async fn main() {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("acceptance_tests=info".parse().unwrap()),
        )
        .init();

    println!("\n=== Angzarr Acceptance Tests ===");
    println!("Testing deployed services via gRPC\n");
    println!("Endpoints:");
    println!(
        "  PLAYER_ENDPOINT: {}",
        std::env::var("PLAYER_ENDPOINT").unwrap_or_else(|_| "http://localhost:1310".to_string())
    );
    println!(
        "  TABLE_ENDPOINT: {}",
        std::env::var("TABLE_ENDPOINT").unwrap_or_else(|_| "http://localhost:1311".to_string())
    );
    println!(
        "  HAND_ENDPOINT: {}",
        std::env::var("HAND_ENDPOINT").unwrap_or_else(|_| "http://localhost:1312".to_string())
    );
    println!();

    // Run end-to-end container tests
    AcceptanceWorld::cucumber()
        .fail_on_skipped()
        .run("tests/acceptance/features/end_to_end.feature")
        .await;
}
