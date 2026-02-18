---
sidebar_position: 1
---

# Why Poker

Poker was chosen as the example domain for ⍼ Angzarr because it naturally exercises **every event sourcing and CQRS pattern** the framework supports—while being immediately understandable.

---

## The Core Insight

Poker has a small vocabulary of event types but generates them as fast as they can be processed. A single hand produces 20+ events in rapid succession: cards dealt, blinds posted, actions taken, community cards revealed, pot awarded. This creates a **challenging scenario** that stress-tests the framework—the kind of rapid-fire event processing that may be sustained in real-world systems during peak load. If ⍼ Angzarr handles poker's pace, it handles production traffic.

A poker application isn't artificially complex—it's exactly as complex as real business software. The patterns that make poker work (fund reservation, saga compensation, state machines, cross-domain coordination) are the same patterns that power airlines, billing systems, and claims processing.

**When you understand how poker works in ⍼ Angzarr, you understand how to build any event-sourced system.**

---

## Industry Applicability

⍼ Angzarr emerged from recognizing recurring patterns across industries. Based on experience working in:

- **Airlines**: Flight booking, seat inventory, loyalty point accrual—all event-sourced naturally. A single booking triggers reservation holds, payment authorization, seat assignment, and loyalty updates. Bursty during booking windows, quiet between.

- **Billing systems**: Invoice generation, payment processing, dunning workflows. Few event types (InvoiceCreated, PaymentReceived, PaymentFailed, DunningEscalated), but generated in monthly billing cycles—massive bursts followed by trickle processing.

- **Insurance claims/preapproval**: Claim submission → document verification → adjuster assignment → approval/denial → payment. State machine with compensation (claim withdrawn, payment reversed). Bursts during enrollment periods and catastrophic events.

In the author's experience, roughly **one-third of projects** across these industries could benefit significantly from CQRS/Event Sourcing—but the infrastructure overhead prevented adoption. ⍼ Angzarr removes that barrier.

### Common Characteristics

Domains that benefit most from ⍼ Angzarr share these traits:

| Characteristic | Example |
|----------------|---------|
| **Audit requirements** | Financial, healthcare, regulated industries |
| **State machine workflows** | Claims, approvals, fulfillment |
| **Cross-domain coordination** | Booking + inventory + payment |
| **Temporal queries** | "What was the balance on March 15?" |
| **Eventual consistency acceptable** | Async processing, read model updates |

---

## Domain Boundaries

Poker naturally decomposes into three bounded contexts:

| Domain | Responsibility | Key Events |
|--------|---------------|------------|
| **Player** | Bankroll, fund reservation | FundsDeposited, FundsReserved, FundsReleased |
| **Table** | Seating, hand orchestration | PlayerJoined, HandStarted, HandEnded |
| **Hand** | Gameplay state machine | CardsDealt, ActionTaken, PotAwarded |

These domains mirror real business separations:
- Player = Accounts/Wallet (financial state)
- Table = Session/Order (coordination layer)
- Hand = Transaction/Workflow (business process)

---

## Pattern Coverage

### 1. Two-Phase Reservation

**Poker**: Player reserves $500 for a table buy-in. Funds are locked but still belong to the player. When the session ends (or fails), funds are released.

**Same pattern applies to**: inventory holds, payment authorizations, hotel bookings, ticket reservations.

### 2. Saga Compensation

**Poker**: Player emits `FundsReserved` → saga issues `JoinTable` → table rejects (full) → player must emit `FundsReleased` to restore available balance.

**Same pattern applies to**: payment→fulfillment rollback, order→inventory release, booking→calendar release.

### 3. State Machine Enforcement

**Poker hand phases**: DEALING → BLINDS → BETTING → FLOP → BETTING → TURN → BETTING → RIVER → BETTING → SHOWDOWN → COMPLETE

**Same pattern applies to**: order fulfillment, insurance claims, approval workflows, onboarding flows.

### 4. Cross-Domain Coordination (Sagas)

**Poker sagas**:
- `saga-table-hand`: HandStarted → DealCards
- `saga-hand-player`: PotAwarded → DepositFunds (winner gets chips)
- `saga-hand-table`: HandComplete → EndHand

**Same pattern applies to**: order→fulfillment, payment→ledger, user→notification.

### 5. Process Manager Orchestration

**Poker PM**: HandFlowPM coordinates the full hand lifecycle across table and hand domains, tracking phase, betting state, and player status.

**Same pattern applies to**: order+payment+shipping coordination, approval chains, multi-step onboarding.

### 6. High-Throughput Event Processing and Snapshots

**Poker**: A single hand generates 20+ events as fast as they can be processed. This sustained high-throughput scenario stress-tests snapshot optimization—without snapshots, replaying thousands of events per command would be unacceptable.

**Testing at poker's pace validates** that the framework handles real-world peak loads:
- Monthly billing cycles (thousands of invoices generated in hours)
- Insurance enrollment periods (surge of applications)
- Airline booking windows (flights opening for sale)
- End-of-quarter processing (financial close activities)

### 7. Optimistic Concurrency

**Poker**: Two players clicking "call" simultaneously must be serialized. Only the player whose turn it is succeeds.

**Same pattern applies to**: inventory allocation, account balance updates, booking confirmations.

---

## Multi-Language Parity

All six language implementations (Python, Go, Rust, Java, C#, C++) run against the **same Gherkin feature files**:

```
examples/features/
├── player.feature      # Player aggregate behavior
├── table.feature       # Table aggregate behavior
├── hand.feature        # Hand aggregate behavior
└── compensation.feature # Cross-domain compensation
```

Each language has its own step definitions, but the specifications are shared. This guarantees identical business behavior across all implementations.

---

## Testing Benefits

Poker provides particularly effective tests because:

| Property | Benefit |
|----------|---------|
| **Clear outcomes** | "Bob wins $100" is easy to verify |
| **Visible effects** | Player balance changes when hand completes |
| **Deterministic replay** | Seeded decks make showdown outcomes predictable |
| **Rich edge cases** | All-in, side pots, elimination—real complexity |
| **Binary states** | Seat 3 either has a player or doesn't |
| **Obvious math** | $500 reserved from $1000 = $500 available |

---

## A Note on Games

Poker makes an excellent *teaching example* because it exercises every pattern. But **games are generally not a good fit for event sourcing in production**:

- Real-time gameplay needs sub-millisecond latency; event sourcing adds overhead
- Game state often depends on timing and physics that don't replay deterministically
- Most games don't need audit trails or temporal queries

The author is developing a board game with ⍼ Angzarr—but primarily because **event logs make game flow understandable during development**. Watching the event stream reveals why a particular game state emerged, which is invaluable for debugging rules and balancing. Whether the production version will use event sourcing remains an open question.

---

## Summary

Every feature file in ⍼ Angzarr's test suite uses poker examples not because poker is special, but because it naturally requires every pattern the framework provides. Understanding the poker domain means understanding:

- How aggregates maintain consistency boundaries
- How sagas translate between domains
- How process managers coordinate long-running workflows
- How compensation handles distributed failures
- How projectors build read models
- How snapshots optimize performance
- How concurrency is managed at scale

**The poker domain is a teaching tool. The patterns transfer to industries where event sourcing genuinely pays off: airlines, billing, insurance, and other domains requiring audit, state machines, and cross-system coordination.**

---

## Next Steps

- **[Aggregates](/examples/aggregates)** — Handler examples in all languages
- **[Sagas](/examples/sagas)** — Cross-domain coordination examples
- **[Testing](/operations/testing)** — Gherkin specifications
