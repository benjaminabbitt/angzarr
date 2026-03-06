# Angzarr Examples

Example implementations demonstrating the Angzarr CQRS/Event Sourcing framework across multiple languages.

## Project Layout

Examples are organized by domain. Each domain directory contains its aggregate and outbound sagas:

```
examples/{lang}/
├── {domain}/
│   ├── agg/              # Domain aggregate
│   └── saga-{target}/    # Saga: this domain → target domain
├── pmg-{name}/           # Process managers (peers to domains)
├── prj-{name}/           # Projectors (peers to domains)
└── tests/
```

### Poker Example Structure

```
examples/rust/                  # (same structure for go/, python/)
├── player/
│   └── agg/                    # Player aggregate (registration, bankroll)
├── table/
│   ├── agg/                    # Table aggregate (seating, game flow)
│   ├── saga-hand/              # Table events → Hand commands
│   └── saga-player/            # Table events → Player commands
├── hand/
│   ├── agg/                    # Hand aggregate (cards, betting, pots)
│   ├── saga-table/             # Hand events → Table commands
│   └── saga-player/            # Hand events → Player commands
├── hand-flow/                  # Cross-domain hand orchestration (PM)
├── prj-output/                 # Event logging projector
└── tests/
```

### Placement Rules

| Component | Location | Example |
|-----------|----------|---------|
| Aggregate | `{domain}/agg/` | `player/agg/`, `table/agg/` |
| Saga | `{source}/saga-{target}/` | `table/saga-hand/` (table→hand) |
| Process Manager | `pmg-{name}/` | `pmg-hand-flow/` |
| Projector | `prj-{name}/` | `prj-output/` |

## Command Runner

This project uses [just](https://github.com/casey/just) as its command runner.

```bash
# Install just
brew install just        # macOS
cargo install just       # any platform with Rust

# List available commands
just

# Run from any language directory
cd rust && just test
cd go && just test
cd python && just test
```

Each example directory is largely self-sufficient. The only external dependency is the angzarr framework binaries/sidecars.

## Domain Model

The poker example implements three bounded contexts:

### Player Aggregate
- **Commands**: RegisterPlayer, DepositFunds, WithdrawFunds, ReserveFunds, ReleaseFunds
- **State**: display_name, email, player_type, bankroll, table_reservations

### Table Aggregate
- **Commands**: CreateTable, JoinTable, LeaveTable, StartHand, EndHand
- **State**: table_name, game_variant, blinds, seats, dealer_position

### Hand Aggregate
- **Commands**: DealCards, PostBlind, PlayerAction, DealCommunityCards, RevealCards, AwardPot
- **State**: deck, player_hands, community_cards, pots, betting_state

## Language Support

### Fully Supported Languages

| Language | Directory | Unit Tests | Acceptance Tests |
|----------|-----------|------------|------------------|
| **Rust** | `rust/` | `#[cfg(test)]` modules | cucumber-rs |
| **Go** | `go/` | `*_test.go` in `tests/` | godog |
| **Python** | `python/` | `test_*.py` in `tests/` | pytest-bdd |
| **Java** | `java/` | JUnit 5 | cucumber-junit5 |
| **C#** | `csharp/` | xUnit | SpecFlow |

### Best-Effort: C++

| Language | Directory | Unit Tests | Acceptance Tests |
|----------|-----------|------------|------------------|
| **C++** | `cpp/` | GTest | cucumber-cpp (broken) |

**C++ is provided as a best-effort example and cannot be supported at the same level as other languages.** The cucumber-cpp project relies on a wire protocol that requires the cucumber-wire Ruby gem, which is incompatible with modern versions of cucumber-ruby. The cucumber-cpp project itself has not been actively maintained since 2021.

**What works:**
- All C++ aggregates, sagas, projectors, and process managers compile correctly
- Step definitions exist and compile into the test binary
- The wire server starts and listens for connections

**What doesn't work:**
- End-to-end Gherkin test execution via cucumber-ruby
- The cucumber-wire gem throws `undefined method 'registry'` errors with cucumber 3.x
- No combination of cucumber/cucumber-wire versions has been found that works with Ruby 3.x

**To verify the build:**
```bash
cd examples/cpp
just build              # Compiles all code including test binary
./build/tests/acceptance_tests --help  # Verify test binary runs
```

If C++ BDD testing is required, consider migrating to a maintained alternative like [Catch2](https://github.com/catchorg/Catch2) with a custom Gherkin parser, or using GTest directly for unit-style testing.

## Acceptance Testing with Gherkin

All examples share [Gherkin](https://cucumber.io/docs/gherkin/) feature files in `features/` for consistent behavior verification:

```gherkin
Scenario: Register a new player
  Given no prior events for the aggregate
  When I handle a RegisterPlayer command with display_name "Alice"
  Then the result is a PlayerRegistered event
```

### Shared Feature Files

Located in `examples/features/`:
- `unit/player.feature` - Player aggregate scenarios
- `unit/table.feature` - Table scenarios
- `unit/hand.feature` - Hand scenarios

## Running Tests

```bash
# From examples/ directory
just test                    # All languages

# Individual languages
cd rust && just test
cd go && just test
cd python && just test
cd java && just test
cd csharp && just test
cd cpp && just build         # C++: build only (see limitations above)
```

## Port Configuration

Each language has a unique port range:

| Language | Range | Player | Table | Hand |
|----------|-------|--------|-------|------|
| Rust | 500xx | 50001 | 50002 | 50003 |
| Go | 502xx | 50201 | 50202 | 50203 |
| Python | 504xx | 50401 | 50402 | 50403 |
| Java | 505xx | 50501 | 50502 | 50503 |
| C# | 506xx | 50601 | 50602 | 50603 |
| C++ | 507xx | 50701 | 50702 | 50703 |
