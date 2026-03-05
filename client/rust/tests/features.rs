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
use steps::aggregate_client::AggregateClientWorld;
use steps::command_builder::CommandBuilderWorld;
use steps::compensation::CompensationWorld;
use steps::connection::ConnectionWorld;
use steps::domain_client::DomainClientWorld;
use steps::error_handling::ErrorHandlingWorld;
use steps::event_decoding::EventDecodingWorld;
use steps::fact_flow::FactFlowWorld;
use steps::merge_strategy::MergeStrategyWorld;
use steps::query_builder::QueryBuilderWorld;
use steps::query_client::QueryClientWorld;
use steps::router::RouterWorld;
use steps::speculative_client::SpeculativeClientWorld;
use steps::state_building::StateBuildingWorld;

#[tokio::main]
async fn main() {
    // Run Connection tests
    println!("\n=== Running Connection Tests ===\n");
    ConnectionWorld::cucumber()
        .fail_on_skipped()
        .run("../features/connection.feature")
        .await;

    // Run DomainClient tests
    println!("\n=== Running DomainClient Tests ===\n");
    DomainClientWorld::cucumber()
        .fail_on_skipped()
        .run("../features/domain-client.feature")
        .await;

    // Run AggregateClient tests
    println!("\n=== Running AggregateClient Tests ===\n");
    AggregateClientWorld::cucumber()
        .fail_on_skipped()
        .run("../features/aggregate_client.feature")
        .await;

    // Run QueryClient tests
    println!("\n=== Running QueryClient Tests ===\n");
    QueryClientWorld::cucumber()
        .fail_on_skipped()
        .run("../features/query_client.feature")
        .await;

    // Run SpeculativeClient tests
    println!("\n=== Running SpeculativeClient Tests ===\n");
    SpeculativeClientWorld::cucumber()
        .fail_on_skipped()
        .run("../features/speculative_client.feature")
        .await;

    // Run FactFlow tests
    println!("\n=== Running FactFlow Tests ===\n");
    FactFlowWorld::cucumber()
        .fail_on_skipped()
        .run("../features/fact_flow.feature")
        .await;

    // Run MergeStrategy tests
    println!("\n=== Running MergeStrategy Tests ===\n");
    MergeStrategyWorld::cucumber()
        .fail_on_skipped()
        .run("../features/merge_strategy.feature")
        .await;

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
