---
sidebar_position: 2
---

# Cucumber / Gherkin

Angzarr uses [Gherkin](https://cucumber.io/docs/gherkin/) feature files as **living specifications** that run against all language implementations.

---

## Why Gherkin

- **Specification as documentation** — Feature files explain WHY behavior exists
- **Language-agnostic** — Same scenarios run against Python, Go, Rust, Java, C#, C++
- **Business-readable** — Non-technical stakeholders can review expected behavior
- **Cross-language consistency** — Guarantees identical behavior across implementations

---

## Feature File Structure

```
examples/features/
├── player.feature           # Player aggregate behavior
├── table.feature            # Table aggregate behavior
├── hand.feature             # Hand aggregate behavior
├── compensation.feature     # Compensation flow patterns
├── saga.feature             # Saga patterns
└── process_manager.feature  # PM patterns
```

---

## Example Feature

```gherkin
@player @aggregate
Feature: Player Aggregate
  The Player aggregate manages bankroll and fund reservations. Funds can be
  deposited, reserved for table buy-ins, and released when sessions end.

  Background:
    Given the angzarr framework is initialized

  # ============================================================================
  # Fund Reservation (Two-Phase Pattern)
  # ============================================================================

  @funds @reservation
  Scenario: Reserve funds for table buy-in
    Given a registered player "Alice" with bankroll 1000
    When Alice reserves 500 for table "Main-1"
    Then Alice's available balance is 500
    And Alice's reserved balance is 500
    And FundsReserved is emitted with:
      | amount   | 500     |
      | table_id | Main-1  |

  @funds @insufficient
  Scenario: Reject reserve when insufficient funds
    Given a registered player "Bob" with bankroll 100
    When Bob tries to reserve 500 for table "Main-1"
    Then the command is rejected with "insufficient_funds"
    And no events are emitted
```

---

## Tags

Use tags to filter test runs:

| Tag | Purpose |
|-----|---------|
| `@player`, `@table`, `@hand` | Filter by domain |
| `@aggregate`, `@saga`, `@pm` | Filter by component type |
| `@reservation`, `@compensation` | Filter by pattern |
| `@wip` | Work in progress (skip in CI) |
| `@slow` | Long-running tests |

```bash
# Run only player tests
behave --tags=@player

# Run reservation tests excluding slow
behave --tags=@reservation --tags=~@slow
```

---

## Step Definitions by Language

Each language implements step definitions for the shared feature files:

### Python (behave)

```python
# features/steps/player_steps.py
from behave import given, when, then

@given('a registered player "{name}" with bankroll {amount:d}')
def step_registered_player(context, name, amount):
    context.player = PlayerState(name=name, bankroll=amount)

@when('{name} reserves {amount:d} for table "{table_id}"')
def step_reserve_funds(context, name, amount, table_id):
    cmd = ReserveFunds(amount=amount, table_id=table_id)
    context.result = handle_reserve(context.player, cmd)

@then("{name}'s available balance is {amount:d}")
def step_check_available(context, name, amount):
    assert context.player.available == amount
```

### Go (godog)

```go
// features/player_steps.go
func (s *Suite) aRegisteredPlayerWithBankroll(name string, amount int) error {
    s.player = &PlayerState{Name: name, Bankroll: amount}
    return nil
}

func (s *Suite) reservesForTable(name string, amount int, tableID string) error {
    cmd := &ReserveFunds{Amount: int32(amount), TableId: tableID}
    event, err := HandleReserve(s.player, cmd)
    s.lastEvent = event
    return err
}

func (s *Suite) availableBalanceIs(name string, amount int) error {
    if s.player.Available != amount {
        return fmt.Errorf("expected %d, got %d", amount, s.player.Available)
    }
    return nil
}
```

### Rust (cucumber-rs)

```rust
// tests/player_steps.rs
#[given(expr = "a registered player {string} with bankroll {int}")]
async fn given_registered_player(world: &mut World, name: String, amount: i32) {
    world.player = PlayerState::new(name, amount);
}

#[when(expr = "{word} reserves {int} for table {string}")]
async fn when_reserve_funds(world: &mut World, _name: String, amount: i32, table_id: String) {
    let cmd = ReserveFunds { amount, table_id };
    world.result = handle_reserve(&world.player, &cmd);
}

#[then(expr = "{word}'s available balance is {int}")]
async fn then_available_balance(world: &mut World, _name: String, amount: i32) {
    assert_eq!(world.player.available, amount);
}
```

---

## Running Tests

```bash
# Python
cd examples/python && behave features/

# Go
cd examples/go && go test -v ./...

# Rust
cd examples/rust && cargo test --test acceptance

# Java
cd examples/java && ./gradlew test

# C#
cd examples/csharp && dotnet test

# C++
cd examples/cpp && ctest
```

---

## Feature File Symlinks

Language-specific directories symlink to the canonical feature files:

```bash
# Setup symlinks (done once)
cd examples/python/features
ln -s ../../features/player.feature .
ln -s ../../features/table.feature .
```

This ensures all languages test against the same specifications.

---

## Next Steps

- **[Testing](/operations/testing)** — Full testing strategy
- **[Why Poker](/examples/why-poker)** — Why poker exercises every pattern
