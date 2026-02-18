# Storage

Event sourcing requires three storage capabilities: persisting events, tracking handler progress, and optionally caching aggregate state. Each serves a distinct purpose in maintaining system correctness and performance.

## EventStore

The EventStore is the source of truth. Every state change in the system exists because an event recorded it. This immutability provides a complete audit trail and enables temporal queries—you can reconstruct any aggregate's state at any point in history.

### Why Strict Sequence Ordering

Events must have consecutive sequences (0, 1, 2, ...) within each aggregate. This constraint exists because:

- **Optimistic concurrency**: Two processes loading the same aggregate and issuing commands would corrupt history if both could write. The second write must fail with a sequence conflict, forcing retry with fresh state.
- **Gap detection**: Missing sequences indicate data corruption or incomplete writes. The system can detect and alert on gaps.
- **Replay ordering**: State reconstruction requires replaying events in order. Ambiguous ordering produces non-deterministic state.

### Why Aggregate Isolation

Events are namespaced by `(domain, edition, root)`. This triple ensures:

- **Bounded contexts**: Domains represent separate business capabilities. A "player" aggregate's events never mix with "table" events.
- **Timeline divergence**: Editions enable what-if analysis and testing. The main timeline ("angzarr") is protected; named editions can diverge and be discarded.
- **Aggregate independence**: Each aggregate root maintains its own event stream. Loading one never touches another's data.

### Why Correlation IDs

Events carry an optional correlation_id linking related events across aggregates. When a saga processes a "HandComplete" event and commands the player system to transfer winnings, both the original event and resulting "FundsDeposited" event share the correlation_id. This enables:

- **Distributed tracing**: Follow a business process across domain boundaries
- **Debugging**: See all effects of a single user action
- **Compensation**: Identify what to undo when a saga fails

## PositionStore

Handlers (projectors, sagas) must remember where they left off. Without position tracking:

- Restarts would reprocess every event from the beginning
- Projectors would corrupt read models with duplicate writes
- Sagas would emit duplicate commands

### Why Per-Handler, Per-Root

Positions are keyed by `(handler, domain, edition, root)`:

- **Handler isolation**: The "player-projector" and "output-projector" process the same events independently. One can be caught up while the other lags.
- **Root isolation**: Processing player-001 to sequence 100 doesn't mean player-002 is processed. Each aggregate root tracks separately.
- **Scaling**: Multiple instances of the same handler can partition work by root without stepping on each other.

### Why Not Event IDs

Positions store sequence numbers, not event IDs or timestamps. Sequences are:

- **Dense**: No gaps means "sequence + 1" reliably identifies the next event
- **Ordered**: Higher sequence = newer event, always
- **Stable**: An event's sequence never changes after write

## SnapshotStore

Aggregates with long histories (thousands of events) become expensive to load. Replaying all events on every command would be unacceptable. Snapshots cache aggregate state at a point-in-time.

### Why Snapshots Are Optional

Snapshots are a performance optimization, not a correctness requirement:

- **Events remain the source of truth**: Snapshots can be deleted without data loss
- **Schema changes**: When aggregate state shape changes, delete snapshots to force replay with new projections
- **Debugging**: Disable snapshot reads to verify event replay produces correct state

### Why Single Snapshot Per Aggregate

Most aggregates need only their latest snapshot. Older snapshots waste storage:

- `put()` atomically stores new snapshot and cleans up transient predecessors
- The exception: `MergeStrategy::Commutative` requires historical snapshots for conflict detection

### Why Sequence-Based

Snapshots record the sequence they reflect. Loading an aggregate:

1. Get snapshot at sequence N
2. Load events from N+1 onwards
3. Apply those events to the snapshot state

If no snapshot exists (or reads are disabled), replay starts from sequence 0.

## Choosing a Backend

| Backend | Durability | Latency | Scaling | Use Case |
|---------|------------|---------|---------|----------|
| PostgreSQL | Strong | Low | Vertical | Production default. ACID guarantees, familiar tooling. |
| SQLite | Strong | Lowest | None | Standalone/embedded mode. Single-file, zero config. |
| Redis | Configurable | Lowest | Horizontal | High-throughput, acceptable loss risk. |
| Bigtable | Strong | Low | Massive | Google Cloud native, petabyte scale. |
| DynamoDB | Strong | Low | Massive | AWS native, serverless scaling. |
| ImmuDB | Cryptographic | Low | Moderate | Audit-critical, tamper-proof requirements. |

## Feature Specifications

The contract each backend must satisfy:

- [EventStore](../../tests/interfaces/features/event_store.feature) - Event persistence, sequencing, concurrency control
- [PositionStore](../../tests/interfaces/features/position_store.feature) - Handler checkpoint tracking
- [SnapshotStore](../../tests/interfaces/features/snapshot_store.feature) - Aggregate state caching

## Running Interface Tests

```bash
# Test against SQLite (default, fast)
cargo test --test interfaces

# Test against specific backend
STORAGE_BACKEND=postgres cargo test --test interfaces
STORAGE_BACKEND=redis cargo test --test interfaces
```

Tests verify every backend implements the same contract. If tests pass on SQLite, they must pass on PostgreSQL, Redis, etc.
