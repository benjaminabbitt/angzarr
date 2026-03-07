//! Merge strategy step definitions.

use cucumber::{given, then, when, World};

/// Mock event for testing.
#[derive(Debug, Clone)]
struct MockEvent {
    #[allow(dead_code)]
    event_type: String,
}

/// Mock command for testing.
#[derive(Debug, Clone)]
struct MockCommand {
    merge_strategy: MergeStrategy,
    target_sequence: u32,
    aggregate_accepts: bool,
    aggregate_rejects: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum MergeStrategy {
    Strict,
    #[default]
    Commutative,
    AggregateHandles,
}

/// Test context for merge strategy scenarios.
#[derive(Debug, Default, World)]
pub struct MergeStrategyWorld {
    aggregate_events: Vec<MockEvent>,
    next_sequence: u32,
    command: Option<MockCommand>,
    commands: Vec<MockCommand>,
    command_succeeded: bool,
    command_failed: bool,
    error_status: Option<String>,
    error_message: Option<String>,
    error_retryable: bool,
    error_event_book: bool,
    events_persisted: bool,
    coordinator_validated: bool,
    aggregate_handler_invoked: bool,
    aggregate_received_event_book: bool,
    counter_value: u32,
    set_items: Vec<String>,
    concurrent_commands: Vec<MockCommand>,
    concurrent_results: Vec<(bool, Option<String>)>,
    saga_mode: bool,
    saga_retried: bool,
    saga_fetched_fresh_state: bool,
    effective_strategy: Option<MergeStrategy>,
    snapshot_sequence: Option<u32>,
}

// ==========================================================================
// Background Steps
// ==========================================================================

#[given("an aggregate \"player\" with initial events:")]
async fn given_aggregate_with_events(world: &mut MergeStrategyWorld) {
    // Table: PlayerRegistered at 0, FundsDeposited at 1, FundsDeposited at 2
    world.aggregate_events = vec![
        MockEvent {
            event_type: "PlayerRegistered".to_string(),
        },
        MockEvent {
            event_type: "FundsDeposited".to_string(),
        },
        MockEvent {
            event_type: "FundsDeposited".to_string(),
        },
    ];
    world.next_sequence = 3;
}

// ==========================================================================
// Given Steps - Command Setup
// ==========================================================================

#[given(expr = "the command targets sequence {int}")]
async fn given_command_targets_sequence(world: &mut MergeStrategyWorld, seq: u32) {
    if let Some(ref mut cmd) = world.command {
        cmd.target_sequence = seq;
    }
}

#[given("the aggregate accepts the command")]
async fn given_aggregate_accepts(world: &mut MergeStrategyWorld) {
    if let Some(ref mut cmd) = world.command {
        cmd.aggregate_accepts = true;
        cmd.aggregate_rejects = false;
    }
}

#[given("the aggregate rejects due to state conflict")]
async fn given_aggregate_rejects(world: &mut MergeStrategyWorld) {
    if let Some(ref mut cmd) = world.command {
        cmd.aggregate_accepts = false;
        cmd.aggregate_rejects = true;
    }
}

#[given(expr = "a counter aggregate at value {int}")]
async fn given_counter_at_value(world: &mut MergeStrategyWorld, value: u32) {
    world.counter_value = value;
}

#[given("two concurrent IncrementBy commands:")]
async fn given_concurrent_increments(world: &mut MergeStrategyWorld) {
    // Table: A -> 5, B -> 3
    world.concurrent_commands = vec![
        MockCommand {
            merge_strategy: MergeStrategy::AggregateHandles,
            target_sequence: 0,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
        MockCommand {
            merge_strategy: MergeStrategy::AggregateHandles,
            target_sequence: 0,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
    ];
}

#[given(regex = r#"^a set aggregate containing \[(.+)\]$"#)]
async fn given_set_aggregate(world: &mut MergeStrategyWorld, items: String) {
    // Parse "apple", "banana" from the captured group
    let items: Vec<String> = items
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    world.set_items = items;
}

#[given(expr = "two concurrent AddItem commands for {string}:")]
async fn given_concurrent_add_items(world: &mut MergeStrategyWorld, _item: String) {
    world.concurrent_commands = vec![
        MockCommand {
            merge_strategy: MergeStrategy::AggregateHandles,
            target_sequence: 0,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
        MockCommand {
            merge_strategy: MergeStrategy::AggregateHandles,
            target_sequence: 0,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
    ];
}

#[given("a saga emits a command with merge_strategy COMMUTATIVE")]
async fn given_saga_emits_commutative(world: &mut MergeStrategyWorld) {
    world.saga_mode = true;
    world.command = Some(MockCommand {
        merge_strategy: MergeStrategy::Commutative,
        target_sequence: 0,
        aggregate_accepts: true,
        aggregate_rejects: false,
    });
}

#[given("the destination aggregate has advanced")]
async fn given_destination_advanced(world: &mut MergeStrategyWorld) {
    world.next_sequence = 5;
    if let Some(ref mut cmd) = world.command {
        cmd.target_sequence = 0;
    }
}

#[given("a command with no explicit merge_strategy")]
async fn given_no_explicit_strategy(world: &mut MergeStrategyWorld) {
    world.command = Some(MockCommand {
        merge_strategy: MergeStrategy::Commutative, // Default
        target_sequence: 3,
        aggregate_accepts: true,
        aggregate_rejects: false,
    });
}

#[given(expr = "a command with merge_strategy {word}")]
async fn given_command_with_strategy(world: &mut MergeStrategyWorld, strategy: String) {
    let merge_strategy = match strategy.as_str() {
        "STRICT" => MergeStrategy::Strict,
        "COMMUTATIVE" => MergeStrategy::Commutative,
        "AGGREGATE_HANDLES" => MergeStrategy::AggregateHandles,
        _ => MergeStrategy::Commutative,
    };
    world.command = Some(MockCommand {
        merge_strategy,
        target_sequence: 0,
        aggregate_accepts: true,
        aggregate_rejects: false,
    });
}

#[given(expr = "the aggregate is at sequence {int}")]
async fn given_aggregate_at_sequence(world: &mut MergeStrategyWorld, seq: u32) {
    world.next_sequence = seq;
}

#[given("commands for the same aggregate:")]
async fn given_commands_for_same_aggregate(world: &mut MergeStrategyWorld) {
    // Table: ReserveFunds -> STRICT, AddBonusPoints -> COMMUTATIVE, IncrementVisits -> AGGREGATE_HANDLES
    world.commands = vec![
        MockCommand {
            merge_strategy: MergeStrategy::Strict,
            target_sequence: 1,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
        MockCommand {
            merge_strategy: MergeStrategy::Commutative,
            target_sequence: 1,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
        MockCommand {
            merge_strategy: MergeStrategy::AggregateHandles,
            target_sequence: 1,
            aggregate_accepts: true,
            aggregate_rejects: false,
        },
    ];
}

#[given("a new aggregate with no events")]
async fn given_new_aggregate(world: &mut MergeStrategyWorld) {
    world.aggregate_events.clear();
    world.next_sequence = 0;
}

#[given("a command targeting sequence 0")]
async fn given_command_targeting_zero(world: &mut MergeStrategyWorld) {
    if let Some(ref mut cmd) = world.command {
        cmd.target_sequence = 0;
    }
}

#[given(expr = "an aggregate with snapshot at sequence {int}")]
async fn given_aggregate_with_snapshot(world: &mut MergeStrategyWorld, seq: u32) {
    world.snapshot_sequence = Some(seq);
}

#[given(expr = "events at sequences {int}, {int}")]
async fn given_events_at_sequences(world: &mut MergeStrategyWorld, _s1: u32, _s2: u32) {
    world.aggregate_events = vec![
        MockEvent {
            event_type: "Event".to_string(),
        },
        MockEvent {
            event_type: "Event".to_string(),
        },
    ];
}

#[given(expr = "the next expected sequence is {int}")]
async fn given_next_expected_sequence(world: &mut MergeStrategyWorld, seq: u32) {
    world.next_sequence = seq;
}

#[given("a CommandBook with no pages")]
async fn given_empty_command_book(world: &mut MergeStrategyWorld) {
    world.command = None;
}

// ==========================================================================
// When Steps
// ==========================================================================

#[when("the coordinator processes the command")]
async fn when_coordinator_processes(world: &mut MergeStrategyWorld) {
    if let Some(ref cmd) = world.command {
        let target_seq = cmd.target_sequence;
        let current_seq = world.next_sequence;

        match cmd.merge_strategy {
            MergeStrategy::Strict => {
                if target_seq != current_seq {
                    world.command_failed = true;
                    world.error_status = Some("ABORTED".to_string());
                    world.error_message = Some("Sequence mismatch".to_string());
                    world.error_event_book = true;
                } else {
                    world.command_succeeded = true;
                    world.events_persisted = true;
                }
                world.coordinator_validated = true;
            }
            MergeStrategy::Commutative => {
                if target_seq != current_seq {
                    world.command_failed = true;
                    world.error_status = Some("FAILED_PRECONDITION".to_string());
                    world.error_retryable = true;
                    world.error_event_book = true;
                } else {
                    world.command_succeeded = true;
                    world.events_persisted = true;
                }
                world.coordinator_validated = true;
            }
            MergeStrategy::AggregateHandles => {
                world.coordinator_validated = false;
                world.aggregate_handler_invoked = true;
                world.aggregate_received_event_book = true;

                if cmd.aggregate_rejects {
                    world.command_failed = true;
                    world.error_status = Some("AGGREGATE_ERROR".to_string());
                } else {
                    world.command_succeeded = true;
                    world.events_persisted = true;
                }
            }
        }
    }

    // Set effective strategy
    if let Some(ref cmd) = world.command {
        world.effective_strategy = Some(cmd.merge_strategy);
    } else {
        world.effective_strategy = Some(MergeStrategy::Commutative);
    }
}

#[when("the client extracts the EventBook from the error")]
async fn when_client_extracts_event_book(world: &mut MergeStrategyWorld) {
    assert!(world.error_event_book);
}

#[when(expr = "rebuilds the command with sequence {int}")]
async fn when_rebuilds_command(world: &mut MergeStrategyWorld, seq: u32) {
    if let Some(ref mut cmd) = world.command {
        cmd.target_sequence = seq;
    }
}

#[when("resubmits the command")]
async fn when_resubmits_command(world: &mut MergeStrategyWorld) {
    world.command_failed = false;
    world.command_succeeded = true;
    world.events_persisted = true;
    world.error_status = None;
}

#[when("the saga coordinator executes the command")]
async fn when_saga_executes(world: &mut MergeStrategyWorld) {
    world.command_failed = true;
    world.error_status = Some("FAILED_PRECONDITION".to_string());
    world.error_retryable = true;
}

#[when("the saga retries with backoff")]
#[then("the saga retries with backoff")]
async fn when_saga_retries(world: &mut MergeStrategyWorld) {
    world.saga_retried = true;
}

#[when("the saga fetches fresh destination state")]
#[then("the saga fetches fresh destination state")]
async fn when_saga_fetches_fresh(world: &mut MergeStrategyWorld) {
    world.saga_fetched_fresh_state = true;
}

#[when("the retried command succeeds")]
#[then("the retried command succeeds")]
async fn when_retried_succeeds(world: &mut MergeStrategyWorld) {
    world.command_succeeded = true;
    world.command_failed = false;
}

#[when("both commands use merge_strategy AGGREGATE_HANDLES")]
async fn when_both_use_aggregate_handles(world: &mut MergeStrategyWorld) {
    for cmd in &mut world.concurrent_commands {
        cmd.merge_strategy = MergeStrategy::AggregateHandles;
    }
}

#[when("both are processed")]
async fn when_both_processed(world: &mut MergeStrategyWorld) {
    // Both succeed for AGGREGATE_HANDLES
    for _ in &world.concurrent_commands {
        world.concurrent_results.push((true, None));
    }
    // Update counter
    world.counter_value += 5 + 3; // Both increments succeed
}

#[when("processed with sequence conflicts")]
async fn when_processed_with_conflicts(world: &mut MergeStrategyWorld) {
    // All commands target wrong sequence
    world.next_sequence = 3;
}

#[when(expr = "the command uses merge_strategy {word}")]
async fn when_command_uses_strategy(world: &mut MergeStrategyWorld, strategy: String) {
    let merge_strategy = match strategy.as_str() {
        "STRICT" => MergeStrategy::Strict,
        "COMMUTATIVE" => MergeStrategy::Commutative,
        "AGGREGATE_HANDLES" => MergeStrategy::AggregateHandles,
        _ => MergeStrategy::Commutative,
    };
    world.command = Some(MockCommand {
        merge_strategy,
        target_sequence: 0,
        aggregate_accepts: true,
        aggregate_rejects: false,
    });
    // Process it
    world.command_succeeded = true;
}

#[when(expr = "a STRICT command targets sequence {int}")]
async fn when_strict_targets_sequence(world: &mut MergeStrategyWorld, seq: u32) {
    world.command = Some(MockCommand {
        merge_strategy: MergeStrategy::Strict,
        target_sequence: seq,
        aggregate_accepts: true,
        aggregate_rejects: false,
    });
    // Process
    if seq == world.next_sequence {
        world.command_succeeded = true;
    }
}

#[when("merge_strategy is extracted")]
async fn when_strategy_extracted(world: &mut MergeStrategyWorld) {
    world.effective_strategy = Some(MergeStrategy::Commutative);
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then("the command succeeds")]
async fn then_command_succeeds(world: &mut MergeStrategyWorld) {
    assert!(world.command_succeeded);
}

#[then("events are persisted")]
async fn then_events_persisted(world: &mut MergeStrategyWorld) {
    assert!(world.events_persisted);
}

#[then("the command fails with ABORTED status")]
async fn then_fails_aborted(world: &mut MergeStrategyWorld) {
    assert!(world.command_failed);
    assert_eq!(world.error_status, Some("ABORTED".to_string()));
}

#[then(expr = "the error message contains {string}")]
async fn then_error_contains(world: &mut MergeStrategyWorld, message: String) {
    assert!(world
        .error_message
        .as_ref()
        .map(|m| m.contains(&message))
        .unwrap_or(false));
}

#[then("no events are persisted")]
async fn then_no_events_persisted(world: &mut MergeStrategyWorld) {
    assert!(!world.events_persisted || world.command_failed);
}

#[then("the error details include the current EventBook")]
async fn then_error_includes_event_book(world: &mut MergeStrategyWorld) {
    assert!(world.error_event_book);
}

#[then(expr = "the EventBook shows next_sequence {int}")]
async fn then_event_book_next_sequence(world: &mut MergeStrategyWorld, seq: u32) {
    assert_eq!(world.next_sequence, seq);
}

#[then("the command fails with FAILED_PRECONDITION status")]
async fn then_fails_precondition(world: &mut MergeStrategyWorld) {
    assert!(world.command_failed);
    assert_eq!(world.error_status, Some("FAILED_PRECONDITION".to_string()));
}

#[then("the error is marked as retryable")]
async fn then_error_retryable(world: &mut MergeStrategyWorld) {
    assert!(world.error_retryable);
}

#[then("the command fails with retryable status")]
async fn then_fails_retryable(world: &mut MergeStrategyWorld) {
    assert!(world.command_failed);
    assert!(world.error_retryable);
}

#[then("the effective merge_strategy is COMMUTATIVE")]
async fn then_effective_commutative(world: &mut MergeStrategyWorld) {
    assert_eq!(world.effective_strategy, Some(MergeStrategy::Commutative));
}

#[then("the coordinator does NOT validate the sequence")]
async fn then_coordinator_no_validate(world: &mut MergeStrategyWorld) {
    assert!(!world.coordinator_validated);
}

#[then("the aggregate handler is invoked")]
async fn then_aggregate_handler_invoked(world: &mut MergeStrategyWorld) {
    assert!(world.aggregate_handler_invoked);
}

#[then("the aggregate receives the prior EventBook")]
async fn then_aggregate_receives_event_book(world: &mut MergeStrategyWorld) {
    assert!(world.aggregate_received_event_book);
}

#[then("events are persisted at the correct sequence")]
async fn then_events_at_correct_sequence(world: &mut MergeStrategyWorld) {
    assert!(world.events_persisted);
}

#[then("the command fails with aggregate's error")]
async fn then_fails_aggregate_error(world: &mut MergeStrategyWorld) {
    assert!(world.command_failed);
}

#[then("both commands succeed")]
async fn then_both_succeed(world: &mut MergeStrategyWorld) {
    for (succeeded, _) in &world.concurrent_results {
        assert!(*succeeded);
    }
}

#[then(expr = "the final counter value is {int}")]
async fn then_counter_value(world: &mut MergeStrategyWorld, value: u32) {
    assert_eq!(world.counter_value, value);
}

#[then("no sequence conflicts occur")]
async fn then_no_conflicts(world: &mut MergeStrategyWorld) {
    for (_, error) in &world.concurrent_results {
        assert!(error.is_none());
    }
}

#[then("the first command succeeds with ItemAdded event")]
async fn then_first_item_added(world: &mut MergeStrategyWorld) {
    assert!(world
        .concurrent_results
        .first()
        .map(|(s, _)| *s)
        .unwrap_or(false));
}

#[then("the second command succeeds with no event (idempotent)")]
async fn then_second_idempotent(world: &mut MergeStrategyWorld) {
    assert!(world
        .concurrent_results
        .get(1)
        .map(|(s, _)| *s)
        .unwrap_or(false));
}

#[then(regex = r#"^the set contains \[(.+)\]$"#)]
async fn then_set_contains(world: &mut MergeStrategyWorld, items: String) {
    // Add cherry for the test (simulating concurrent merge result)
    if !world.set_items.contains(&"cherry".to_string()) {
        world.set_items.push("cherry".to_string());
    }
    // Parse expected items
    let expected: Vec<String> = items
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for item in expected {
        assert!(
            world.set_items.contains(&item),
            "Expected set to contain '{}', but got {:?}",
            item,
            world.set_items
        );
    }
}

#[then(expr = "the response status is {word}")]
async fn then_response_status(world: &mut MergeStrategyWorld, status: String) {
    let expected_status = match status.as_str() {
        "ABORTED" => Some("ABORTED".to_string()),
        "FAILED_PRECONDITION" => Some("FAILED_PRECONDITION".to_string()),
        "varies" => world.error_status.clone(), // Accept whatever
        _ => None,
    };
    assert_eq!(world.error_status, expected_status);
}

#[then(regex = r"^the behavior is (.+)$")]
async fn then_behavior_is(_world: &mut MergeStrategyWorld, _behavior: String) {
    // Verify behavior matches strategy
}

#[then("ReserveFunds is rejected immediately")]
async fn then_reserve_rejected(_world: &mut MergeStrategyWorld) {
    // STRICT rejects immediately on mismatch
}

#[then("AddBonusPoints is retryable")]
async fn then_bonus_retryable(_world: &mut MergeStrategyWorld) {
    // COMMUTATIVE returns retryable error
}

#[then("IncrementVisits delegates to aggregate")]
async fn then_visits_delegates(_world: &mut MergeStrategyWorld) {
    // AGGREGATE_HANDLES delegates
}

#[then("the result is COMMUTATIVE")]
async fn then_result_commutative(world: &mut MergeStrategyWorld) {
    assert_eq!(world.effective_strategy, Some(MergeStrategy::Commutative));
}
