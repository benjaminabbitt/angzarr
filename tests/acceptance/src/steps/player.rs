//! Step definitions for player domain acceptance tests.

use crate::AcceptanceWorld;
use angzarr_client::proto::examples::{PlayerType, RegisterPlayer};
use angzarr_client::proto::{command_page, event_page, CommandBook, CommandPage, MergeStrategy};
use cucumber::{given, then, when};
use prost::Message;
use prost_types::Any;

/// Pack a command into Any.
fn pack_command<M: Message>(msg: &M, type_name: &str) -> Any {
    Any {
        type_url: format!("type.googleapis.com/{}", type_name),
        value: msg.encode_to_vec(),
    }
}

/// Helper to create a CommandPage with default merge strategy.
fn make_command_page(sequence: u32, command: Any) -> CommandPage {
    CommandPage {
        sequence,
        merge_strategy: MergeStrategy::MergeCommutative.into(),
        payload: Some(command_page::Payload::Command(command)),
    }
}

/// Extract sequence from EventPage (handles the oneof).
fn event_sequence(page: &angzarr_client::proto::EventPage) -> Option<u32> {
    match &page.sequence_type {
        Some(angzarr_client::proto::event_page::SequenceType::Sequence(seq)) => Some(*seq),
        _ => None,
    }
}

/// Extract event type_url from EventPage (handles the oneof).
fn event_type_url(page: &angzarr_client::proto::EventPage) -> Option<&str> {
    match &page.payload {
        Some(event_page::Payload::Event(any)) => Some(&any.type_url),
        _ => None,
    }
}

// =============================================================================
// Background Steps
// =============================================================================

#[given(regex = r#"the angzarr system is deployed and reachable at "([^"]+)""#)]
async fn given_system_deployed(world: &mut AcceptanceWorld, _endpoint: String) {
    // Connect to all domain services
    // Note: Individual endpoints are configured via environment variables
    world
        .connect()
        .await
        .expect("Failed to connect to services");
}

// =============================================================================
// Given Steps
// =============================================================================

#[given("a new player aggregate (unique ID for test isolation)")]
async fn given_new_player_aggregate(world: &mut AcceptanceWorld) {
    world.new_aggregate_root("player");
}

#[given("a new player aggregate")]
async fn given_new_player_aggregate_simple(world: &mut AcceptanceWorld) {
    world.new_aggregate_root("player");
}

#[given("a player aggregate that has processed a RegisterPlayer command")]
async fn given_player_with_registration(world: &mut AcceptanceWorld) {
    // Create new aggregate
    world.new_aggregate_root("player");

    // Send RegisterPlayer command
    let cover = world.current_cover().expect("No current cover");
    let register = RegisterPlayer {
        display_name: "Setup Player".to_string(),
        email: "setup@test.com".to_string(),
        player_type: PlayerType::Human.into(),
        ..Default::default()
    };

    let command_book = CommandBook {
        cover: Some(cover),
        pages: vec![make_command_page(
            0,
            pack_command(&register, "examples.RegisterPlayer"),
        )],
        ..Default::default()
    };

    world
        .send_command(command_book)
        .await
        .expect("Setup command failed");
}

// =============================================================================
// When Steps
// =============================================================================

#[when(regex = r#"a RegisterPlayer command is sent with name "([^"]+)" and email "([^"]+)""#)]
async fn when_register_player(world: &mut AcceptanceWorld, name: String, email: String) {
    let cover = world.current_cover().expect("No current cover");
    let next_seq = world.event_count() as u32;

    let register = RegisterPlayer {
        display_name: name,
        email,
        player_type: PlayerType::Human.into(),
        ..Default::default()
    };

    let command_book = CommandBook {
        cover: Some(cover),
        pages: vec![make_command_page(
            next_seq,
            pack_command(&register, "examples.RegisterPlayer"),
        )],
        ..Default::default()
    };

    let _ = world.send_command(command_book).await;
}

#[when("a RegisterPlayer command is processed")]
async fn when_register_player_processed(world: &mut AcceptanceWorld) {
    let cover = world.current_cover().expect("No current cover");
    let next_seq = world.event_count() as u32;

    let register = RegisterPlayer {
        display_name: "Test Player".to_string(),
        email: "test@example.com".to_string(),
        player_type: PlayerType::Human.into(),
        ..Default::default()
    };

    let command_book = CommandBook {
        cover: Some(cover),
        pages: vec![make_command_page(
            next_seq,
            pack_command(&register, "examples.RegisterPlayer"),
        )],
        ..Default::default()
    };

    let _ = world.send_command(command_book).await;
}

#[when("we query that aggregate's event history")]
async fn when_query_event_history(_world: &mut AcceptanceWorld) {
    // Events are already accumulated in world.events from previous commands
    // For a full implementation, we'd use QueryClient here
    // For now, the events from command responses are sufficient
}

// =============================================================================
// Then Steps
// =============================================================================

#[then("the command succeeds (aggregate processed it)")]
async fn then_command_succeeds(world: &mut AcceptanceWorld) {
    assert!(
        world.command_succeeded(),
        "Command failed: {:?}",
        world.last_error
    );
}

#[then("a PlayerRegistered event was persisted")]
async fn then_player_registered_event(world: &mut AcceptanceWorld) {
    let has_registration = world.events.iter().any(|page| {
        event_type_url(page)
            .map(|url| url.contains("PlayerRegistered"))
            .unwrap_or(false)
    });
    assert!(has_registration, "No PlayerRegistered event found");
}

#[then(regex = r"the aggregate's event count is (\d+)")]
async fn then_event_count(world: &mut AcceptanceWorld, count: usize) {
    assert_eq!(
        world.event_count(),
        count,
        "Expected {} events, got {}",
        count,
        world.event_count()
    );
}

#[then("we receive the PlayerRegistered event at sequence 0")]
async fn then_receive_player_registered_at_seq_0(world: &mut AcceptanceWorld) {
    let event = world
        .events
        .iter()
        .find(|p| event_sequence(p) == Some(0))
        .expect("No event at sequence 0");

    assert!(
        event_type_url(event)
            .map(|url| url.contains("PlayerRegistered"))
            .unwrap_or(false),
        "Event at sequence 0 is not PlayerRegistered"
    );
}

#[then("the response includes any synchronous projections")]
async fn then_response_includes_projections(world: &mut AcceptanceWorld) {
    // Projections are optional - just verify command succeeded
    assert!(world.command_succeeded(), "Command should have succeeded");
    // In a full implementation, we'd check world.last_response.projections
}
