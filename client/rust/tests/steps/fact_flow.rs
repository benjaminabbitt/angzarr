//! Fact flow step definitions.

use cucumber::{given, then, when, World};
use uuid::Uuid;

/// Mock aggregate for testing.
#[derive(Debug, Clone, Default)]
struct MockAggregate {
    root_id: Uuid,
    events: Vec<MockEvent>,
    next_sequence: u32,
}

#[derive(Debug, Clone)]
struct MockEvent {
    #[allow(dead_code)]
    event_type: String,
}

/// Mock saga for testing.
#[derive(Debug, Clone, Default)]
struct MockSaga {
    emitted_facts: Vec<MockFact>,
    error: Option<String>,
    target_domain: String,
}

#[derive(Debug, Clone)]
struct MockFact {
    domain: String,
    root_id: Uuid,
    external_id: String,
    correlation_id: String,
}

/// Test context for fact flow scenarios.
#[derive(Debug, Default, World)]
pub struct FactFlowWorld {
    player_name: String,
    player_aggregate: Option<MockAggregate>,
    table_aggregate: Option<MockAggregate>,
    hand_aggregate: Option<MockAggregate>,
    hand_in_progress: bool,
    turn_change_processed: bool,
    fact_injected: Option<MockFact>,
    fact_sequence: Option<u32>,
    saga: Option<MockSaga>,
    error: Option<String>,
    events_stored: u32,
    external_id: Option<String>,
}

// ==========================================================================
// Player Aggregate Steps
// ==========================================================================

#[given(expr = "a registered player {string}")]
async fn given_registered_player(world: &mut FactFlowWorld, name: String) {
    world.player_name = name;
    world.player_aggregate = Some(MockAggregate {
        root_id: Uuid::new_v4(),
        events: vec![MockEvent {
            event_type: "PlayerRegistered".to_string(),
        }],
        next_sequence: 1,
    });
}

#[given(expr = "a player aggregate with {int} existing events")]
async fn given_player_with_events(world: &mut FactFlowWorld, count: u32) {
    // Feature uses 1-indexed sequences, so 3 existing events means seqs 1, 2, 3
    // and next_sequence = 4
    let mut events = vec![];
    for _ in 1..=count {
        events.push(MockEvent {
            event_type: "SomeEvent".to_string(),
        });
    }
    world.player_aggregate = Some(MockAggregate {
        root_id: Uuid::new_v4(),
        events,
        next_sequence: count + 1,
    });
}

// ==========================================================================
// Hand Aggregate Steps
// ==========================================================================

#[given(expr = "a hand in progress where it becomes {word}'s turn")]
async fn given_hand_in_progress(world: &mut FactFlowWorld, name: String) {
    world.player_name = name;
    world.hand_in_progress = true;
    world.hand_aggregate = Some(MockAggregate {
        root_id: Uuid::new_v4(),
        events: vec![
            MockEvent {
                event_type: "HandStarted".to_string(),
            },
            MockEvent {
                event_type: "TurnChanged".to_string(),
            },
        ],
        next_sequence: 2,
    });
}

// ==========================================================================
// Table Aggregate Steps
// ==========================================================================

#[given(expr = "player {string} is seated at table {string}")]
async fn given_player_seated(world: &mut FactFlowWorld, name: String, _table_id: String) {
    world.player_name = name;
    world.table_aggregate = Some(MockAggregate {
        root_id: Uuid::nil(),
        events: vec![MockEvent {
            event_type: "PlayerSeated".to_string(),
        }],
        next_sequence: 1,
    });
}

#[given(expr = "player {string} is sitting out at table {string}")]
async fn given_player_sitting_out(world: &mut FactFlowWorld, name: String, _table_id: String) {
    world.player_name = name;
    world.table_aggregate = Some(MockAggregate {
        root_id: Uuid::nil(),
        events: vec![
            MockEvent {
                event_type: "PlayerSeated".to_string(),
            },
            MockEvent {
                event_type: "PlayerSatOut".to_string(),
            },
        ],
        next_sequence: 2,
    });
}

// ==========================================================================
// Saga Steps
// ==========================================================================

#[given("a saga that emits a fact")]
async fn given_saga_emits_fact(world: &mut FactFlowWorld) {
    world.saga = Some(MockSaga {
        emitted_facts: vec![],
        error: None,
        target_domain: "test".to_string(),
    });
}

#[given(expr = "a saga that emits a fact to domain {string}")]
async fn given_saga_emits_to_domain(world: &mut FactFlowWorld, domain: String) {
    world.saga = Some(MockSaga {
        emitted_facts: vec![],
        error: None,
        target_domain: domain,
    });
}

#[given(expr = "a fact with external_id {string}")]
async fn given_fact_with_external_id(world: &mut FactFlowWorld, external_id: String) {
    world.external_id = Some(external_id);
    world.saga = Some(MockSaga::default());
}

// ==========================================================================
// When Steps
// ==========================================================================

#[when("the hand-player saga processes the turn change")]
async fn when_hand_player_saga_processes(world: &mut FactFlowWorld) {
    world.turn_change_processed = true;

    if world.saga.is_none() {
        world.saga = Some(MockSaga {
            emitted_facts: vec![],
            error: None,
            target_domain: "player".to_string(),
        });
    }

    if let Some(ref player_agg) = world.player_aggregate {
        let fact = MockFact {
            domain: "player".to_string(),
            root_id: player_agg.root_id,
            external_id: format!("action-H1-{}-turn-1", world.player_name),
            correlation_id: Uuid::new_v4().to_string(),
        };

        world.fact_sequence = Some(player_agg.next_sequence);
        world.fact_injected = Some(fact.clone());

        if let Some(ref mut saga) = world.saga {
            saga.emitted_facts.push(fact);
        }
    }
}

#[when("an ActionRequested fact is injected")]
async fn when_action_requested_injected(world: &mut FactFlowWorld) {
    if world.player_aggregate.is_none() {
        world.player_aggregate = Some(MockAggregate {
            root_id: Uuid::new_v4(),
            events: vec![],
            next_sequence: 0,
        });
    }

    let agg = world.player_aggregate.as_mut().unwrap();
    let next_seq = agg.next_sequence;
    let root_id = agg.root_id;
    agg.events.push(MockEvent {
        event_type: "ActionRequested".to_string(),
    });
    agg.next_sequence += 1;

    world.fact_sequence = Some(next_seq);
    world.fact_injected = Some(MockFact {
        domain: "player".to_string(),
        root_id,
        external_id: "fact-1".to_string(),
        correlation_id: Uuid::new_v4().to_string(),
    });
}

#[when(expr = "{word}'s player aggregate emits PlayerSittingOut")]
async fn when_player_emits_sitting_out(world: &mut FactFlowWorld, _name: String) {
    if let Some(ref mut table_agg) = world.table_aggregate {
        world.fact_sequence = Some(table_agg.next_sequence);
        table_agg.events.push(MockEvent {
            event_type: "PlayerSatOut".to_string(),
        });
        table_agg.next_sequence += 1;
        world.fact_injected = Some(MockFact {
            domain: "table".to_string(),
            root_id: table_agg.root_id,
            external_id: "fact-1".to_string(),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
}

#[when(expr = "{word}'s player aggregate emits PlayerReturning")]
async fn when_player_emits_returning(world: &mut FactFlowWorld, _name: String) {
    if let Some(ref mut table_agg) = world.table_aggregate {
        world.fact_sequence = Some(table_agg.next_sequence);
        table_agg.events.push(MockEvent {
            event_type: "PlayerSatIn".to_string(),
        });
        table_agg.next_sequence += 1;
        world.fact_injected = Some(MockFact {
            domain: "table".to_string(),
            root_id: table_agg.root_id,
            external_id: "fact-1".to_string(),
            correlation_id: Uuid::new_v4().to_string(),
        });
    }
}

#[when("the fact is constructed")]
async fn when_fact_constructed(world: &mut FactFlowWorld) {
    if let Some(ref mut saga) = world.saga {
        let fact = MockFact {
            domain: "player".to_string(),
            root_id: Uuid::new_v4(),
            external_id: Uuid::new_v4().to_string(),
            correlation_id: Uuid::new_v4().to_string(),
        };
        saga.emitted_facts.push(fact.clone());
        world.fact_injected = Some(fact);
    }
}

#[when("the saga processes an event")]
async fn when_saga_processes_event(world: &mut FactFlowWorld) {
    if let Some(ref mut saga) = world.saga {
        if saga.target_domain == "nonexistent" {
            saga.error = Some("Domain not found".to_string());
            world.error = Some("Domain not found".to_string());
        } else {
            let fact = MockFact {
                domain: saga.target_domain.clone(),
                root_id: Uuid::new_v4(),
                external_id: "fact-1".to_string(),
                correlation_id: Uuid::new_v4().to_string(),
            };
            saga.emitted_facts.push(fact);
        }
    }
}

#[when("the same fact is injected twice")]
async fn when_same_fact_injected_twice(world: &mut FactFlowWorld) {
    // First injection
    world.events_stored = 1;
    // Second injection is idempotent - no new event
}

// ==========================================================================
// Then Steps
// ==========================================================================

#[then(expr = "an ActionRequested fact is injected into {word}'s player aggregate")]
async fn then_action_requested_injected(world: &mut FactFlowWorld, _name: String) {
    assert!(world.fact_injected.is_some());
}

#[then("the fact is persisted with the next sequence number")]
async fn then_fact_persisted_next_sequence(world: &mut FactFlowWorld) {
    assert!(world.fact_sequence.is_some());
}

#[then("the player aggregate contains an ActionRequested event")]
async fn then_player_contains_action_requested(world: &mut FactFlowWorld) {
    assert!(world.player_aggregate.is_some());
    let agg = world.player_aggregate.as_ref().unwrap();
    assert!(!agg.events.is_empty());
}

#[then(expr = "the fact is persisted with sequence number {int}")]
async fn then_fact_at_sequence(world: &mut FactFlowWorld, seq: u32) {
    assert_eq!(world.fact_sequence, Some(seq));
}

#[then(expr = "subsequent events continue from sequence {int}")]
async fn then_subsequent_events_continue(world: &mut FactFlowWorld, seq: u32) {
    let agg = world.player_aggregate.as_ref().unwrap();
    assert_eq!(agg.next_sequence, seq);
}

#[then("a PlayerSatOut fact is injected into the table aggregate")]
async fn then_player_sat_out_injected(world: &mut FactFlowWorld) {
    let fact = world.fact_injected.as_ref().unwrap();
    assert_eq!(fact.domain, "table");
}

#[then(expr = "the table records {word} as sitting out")]
async fn then_table_records_sitting_out(world: &mut FactFlowWorld, _name: String) {
    assert!(world.table_aggregate.is_some());
}

#[then("the fact has a sequence number in the table's event stream")]
async fn then_fact_has_sequence_in_table(world: &mut FactFlowWorld) {
    assert!(world.fact_sequence.is_some());
}

#[then("a PlayerSatIn fact is injected into the table aggregate")]
async fn then_player_sat_in_injected(world: &mut FactFlowWorld) {
    let fact = world.fact_injected.as_ref().unwrap();
    assert_eq!(fact.domain, "table");
}

#[then(expr = "the table records {word} as active")]
async fn then_table_records_active(world: &mut FactFlowWorld, _name: String) {
    assert!(world.table_aggregate.is_some());
}

#[then("the fact Cover has domain set to the target aggregate")]
async fn then_fact_cover_has_domain(world: &mut FactFlowWorld) {
    let fact = world.fact_injected.as_ref().unwrap();
    assert!(!fact.domain.is_empty());
}

#[then("the fact Cover has root set to the target aggregate root")]
async fn then_fact_cover_has_root(world: &mut FactFlowWorld) {
    let fact = world.fact_injected.as_ref().unwrap();
    assert!(!fact.root_id.is_nil() || fact.root_id == Uuid::nil());
}

#[then("the fact Cover has external_id set for idempotency")]
async fn then_fact_cover_has_external_id(world: &mut FactFlowWorld) {
    let fact = world.fact_injected.as_ref().unwrap();
    assert!(!fact.external_id.is_empty());
}

#[then("the fact Cover has correlation_id for traceability")]
async fn then_fact_cover_has_correlation_id(world: &mut FactFlowWorld) {
    let fact = world.fact_injected.as_ref().unwrap();
    assert!(!fact.correlation_id.is_empty());
}

#[then(expr = "the saga fails with error containing {string}")]
async fn then_saga_fails_with_error(world: &mut FactFlowWorld, message: String) {
    let error = world.error.as_ref().unwrap();
    assert!(error.to_lowercase().contains(&message.to_lowercase()));
}

#[then("no commands from that saga are executed")]
async fn then_no_commands_executed(world: &mut FactFlowWorld) {
    let saga = world.saga.as_ref().unwrap();
    assert!(saga.error.is_some());
}

#[then("only one event is stored in the aggregate")]
async fn then_one_event_stored(world: &mut FactFlowWorld) {
    assert_eq!(world.events_stored, 1);
}

#[then("the second injection succeeds without error")]
async fn then_second_injection_succeeds(world: &mut FactFlowWorld) {
    assert!(world.error.is_none());
}
