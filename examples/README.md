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
├── pmg-hand-flow/              # Cross-domain hand orchestration
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

### Primary Languages

| Language | Directory | Unit Tests | Acceptance Tests |
|----------|-----------|------------|------------------|
| **Rust** | `rust/` | `#[cfg(test)]` modules | cucumber-rs |
| **Go** | `go/` | `*_test.go` in `tests/` | godog |
| **Python** | `python/` | `test_*.py` in `tests/` | pytest-bdd |

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
```

## Port Configuration

Each language has a unique port range:

| Language | Range | Player | Table | Hand |
|----------|-------|--------|-------|------|
| Rust | 500xx | 50001 | 50002 | 50003 |
| Go | 502xx | 50201 | 50202 | 50203 |
| Python | 504xx | 50401 | 50402 | 50403 |
