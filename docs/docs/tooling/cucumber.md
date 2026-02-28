---
sidebar_position: 2
---

# Cucumber / Gherkin

Angzarr uses [Gherkin](https://cucumber.io/docs/gherkin/) feature files as **living specifications** that run against all language implementations.

---

## Two Testing Approaches

Angzarr uses different strategies for client libraries vs example implementations:

### Client Libraries: Unified Rust Harness

Client libraries (`client/{lang}/`) are tested with a **single Rust Gherkin harness** via gRPC:

```
┌─────────────────────────────────────────────────────────────┐
│  Rust Gherkin Harness (cucumber-rs)                         │
│  - Step definitions: tests/client/                          │
│  - Feature files: client/features/*.feature                 │
└─────────────────┬───────────────────────────────────────────┘
                  │ gRPC
    ┌─────────────┼─────────────┬─────────────┬───────────────┐
    ▼             ▼             ▼             ▼               ▼
┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐
│ Python │  │  Go    │  │  Rust  │  │  Java  │  │  C#    │  │  C++   │
│ Client │  │ Client │  │ Client │  │ Client │  │ Client │  │ Client │
└────────┘  └────────┘  └────────┘  └────────┘  └────────┘  └────────┘
```

**Why unified?**
- One source of truth for SDK contracts
- Same tests validate all implementations
- Tests actual gRPC protocol

```bash
just test-client python    # Test Python client
just test-client go        # Test Go client
just test-clients          # Test all clients
```

### Example Implementations: Per-Language Harnesses

Example business logic (`examples/{lang}/`) uses **per-language Gherkin harnesses**:

```
examples/features/unit/*.feature  (shared specifications)
           │
           ├── Python: behave + examples/python/features/steps/
           ├── Go: godog + examples/go/tests/steps/
           ├── Rust: cucumber-rs + examples/rust/tests/
           ├── Java: cucumber-junit5 + examples/java/tests/
           ├── C#: SpecFlow + examples/csharp/Tests/Steps/
           └── C++: cucumber-cpp + examples/cpp/tests/
```

**Why per-language?**
- Demonstrative for non-polyglot developers
- Developers see Gherkin + step definitions in their language
- Educational code they can learn from

```bash
just examples python test  # behave
just examples go test      # godog
just examples rust test    # cucumber-rs
just examples java test    # cucumber-junit5
just examples csharp test  # SpecFlow
just examples cpp test     # cucumber-cpp
```

---

## Feature File Structure

Feature files are shared specifications:

```
examples/features/
├── unit/
│   ├── player.feature           # Player aggregate behavior
│   ├── table.feature            # Table aggregate behavior
│   ├── hand.feature             # Hand aggregate behavior
│   ├── saga.feature             # Saga patterns
│   ├── process_manager.feature  # PM patterns
│   ├── projector.feature        # Projector patterns
│   └── ...
└── acceptance/
    └── poker_game.feature       # End-to-end poker flow

client/features/
├── aggregate-client.feature     # Aggregate client contracts
├── command-builder.feature      # Command builder contracts
├── query-client.feature         # Query client contracts
└── ...
```

---

## Example Feature

```gherkin
@player @aggregate
Feature: Player Aggregate
  The Player aggregate manages bankroll and fund reservations.

  Background:
    Given the angzarr framework is initialized

  @funds @reservation
  Scenario: Reserve funds for table buy-in
    Given a registered player "Alice" with bankroll 1000
    When Alice reserves 500 for table "Main-1"
    Then Alice's available balance is 500
    And Alice's reserved balance is 500

  @funds @insufficient
  Scenario: Reject reserve when insufficient funds
    Given a registered player "Bob" with bankroll 100
    When Bob tries to reserve 500 for table "Main-1"
    Then the command is rejected with "insufficient_funds"
```

---

## Tags

| Tag | Purpose |
|-----|---------|
| `@player`, `@table`, `@hand` | Filter by domain |
| `@aggregate`, `@saga`, `@pm` | Filter by component type |
| `@reservation`, `@compensation` | Filter by pattern |
| `@wip` | Work in progress (skip in CI) |
| `@slow` | Long-running tests |

---

## Step Definition Examples

### Python (behave) - Examples

```python
# examples/python/features/steps/player_steps.py
from behave import given, when, then

@given('a registered player "{name}" with bankroll {amount:d}')
def step_registered_player(context, name, amount):
    context.player = PlayerState(name=name, bankroll=amount)

@when('{name} reserves {amount:d} for table "{table_id}"')
def step_reserve_funds(context, name, amount, table_id):
    cmd = ReserveFunds(amount=amount, table_id=table_id)
    context.result = handle_reserve(context.player, cmd)
```

### Rust (cucumber-rs) - Client Harness

```rust
// tests/client/steps/aggregate.rs
#[given(expr = "a registered player {string} with bankroll {int}")]
async fn given_registered_player(world: &mut World, name: String, amount: i32) {
    // Call client via gRPC
    let response = world.client
        .register_player(RegisterPlayerRequest { name, initial_bankroll: amount })
        .await
        .expect("gRPC call failed");
    world.player_id = response.player_id;
}
```

---

## Summary

| Component | Harness | Reason |
|-----------|---------|--------|
| `client/{lang}/` | Unified Rust gRPC | SDK contract testing, one source of truth |
| `examples/{lang}/` | Per-language | Demonstrative, educational for developers |

---

## Next Steps

- **[Testing](/operations/testing)** — Full testing strategy
- **[Why Poker](/examples/why-poker)** — Why poker exercises every pattern
