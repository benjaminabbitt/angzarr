//! Cucumber feature tests for the angzarr-client library.
//!
//! These tests verify client library behavior using Gherkin scenarios.
//! Run with:
//!
//! ```bash
//! cargo test --test features
//! ```

mod steps;

use cucumber::World;
use steps::command_builder::CommandBuilderWorld;
use steps::compensation::CompensationWorld;
use steps::error_handling::ErrorHandlingWorld;
use steps::event_decoding::EventDecodingWorld;
use steps::query_builder::QueryBuilderWorld;
use steps::router::RouterWorld;
use steps::state_building::StateBuildingWorld;

#[tokio::main]
async fn main() {
    // Run CommandBuilder tests
    println!("\n=== Running CommandBuilder Tests ===\n");
    CommandBuilderWorld::cucumber()
        .fail_on_skipped()
        .run("../features/command_builder.feature")
        .await;

    // Run QueryBuilder tests
    println!("\n=== Running QueryBuilder Tests ===\n");
    QueryBuilderWorld::cucumber()
        .fail_on_skipped()
        .run("../features/query_builder.feature")
        .await;

    // Run ErrorHandling tests
    println!("\n=== Running ErrorHandling Tests ===\n");
    ErrorHandlingWorld::cucumber()
        .fail_on_skipped()
        .run("../features/error_handling.feature")
        .await;

    // Run Router tests
    println!("\n=== Running Router Tests ===\n");
    RouterWorld::cucumber()
        .fail_on_skipped()
        .run("../features/router.feature")
        .await;

    // Run StateBuildingWorld tests
    println!("\n=== Running StateBuilding Tests ===\n");
    StateBuildingWorld::cucumber()
        .fail_on_skipped()
        .run("../features/state_building.feature")
        .await;

    // Run EventDecoding tests
    println!("\n=== Running EventDecoding Tests ===\n");
    EventDecodingWorld::cucumber()
        .fail_on_skipped()
        .run("../features/event_decoding.feature")
        .await;

    // Run Compensation tests
    println!("\n=== Running Compensation Tests ===\n");
    CompensationWorld::cucumber()
        .fail_on_skipped()
        .run("../features/compensation.feature")
        .await;
}
