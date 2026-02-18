# ⍼ Angzarr

A CQRS/Event Sourcing infrastructure framework in Rust.

## Overview

Angzarr provides the infrastructure layer for event-sourced systems:
- Event persistence with sequence validation
- Snapshot optimization for aggregate replay
- Event upcasting / schema evolution
- gRPC event distribution
- Projector and saga coordination

client logic runs as external gRPC services — any language with gRPC support works. Most teams pick one language and stick with it; the value is that the choice is entirely theirs. Domain code *may* import Angzarr client libraries to simplify development, but this is not required — the only contract is gRPC + protobuf. See [angzarr-client](angzarr-client/rust/) for the Rust client library.

## Why Angzarr

CQRS/Event Sourcing delivers powerful benefits -- complete audit trails, temporal queries, scalable read models -- but the infrastructure complexity is prohibitive. Teams need expertise in event stores, snapshot optimization, distributed messaging, saga coordination, and concurrency control before writing a single line of client logic.

Angzarr inverts this equation: **client logic becomes the only thing you write**.

### The Core Insight

client logic should be pure functions: `(state, command) -> events`. When you strip away infrastructure concerns, what remains is exactly what junior developers and AI code generators excel at:

- Clear input/output contracts (protobuf schemas)
- No side effects to reason about
- No concurrency bugs possible (single aggregate, sequential events)
- Testable in isolation with simple assertions

### The Pit of Success

The architecture makes incorrect code difficult to write. A developer literally *cannot*:

| Mistake | Why It's Impossible |
|---------|---------------------|
| Introduce race conditions | Single aggregate processes commands sequentially |
| Corrupt database transactions | Angzarr manages all persistence |
| Create connection pool exhaustion | No direct database access |
| Accidentally expose internal state | State is reconstructed from events |
| Break other aggregates | Aggregates are isolated by design |

They write a function. It either returns the correct events or it doesn't. That's testable with unit tests against the handler alone.

### What Junior Devs and AIs Write

Every handler follows the same mechanical pattern:

```
1. GUARD:    Check preconditions against current state
2. VALIDATE: Check command field validity
3. COMPUTE:  Calculate derived values (pure math)
4. EMIT:     Return event(s) describing what happened
```

Here's the same client logic in three languages -- notice the structural identity:

**Go** (24 lines of logic):
```go
func (l *DefaultCartLogic) HandleAddItem(state *CartState, productID, name string,
    quantity, unitPriceCents int32) (*examples.ItemAdded, error) {
    // GUARD
    if !state.Exists() {
        return nil, NewFailedPrecondition(ErrMsgCartNotFound)
    }
    if !state.IsActive() {
        return nil, NewFailedPrecondition(ErrMsgCartCheckedOut)
    }
    // VALIDATE
    if productID == "" {
        return nil, NewInvalidArgument(ErrMsgProductIDRequired)
    }
    if quantity <= 0 {
        return nil, NewInvalidArgument(ErrMsgQuantityPositive)
    }
    // COMPUTE + EMIT
    newSubtotal := state.SubtotalCents + (quantity * unitPriceCents)
    return &examples.ItemAdded{
        ProductId:      productID,
        Name:           name,
        Quantity:       quantity,
        UnitPriceCents: unitPriceCents,
        NewSubtotal:    newSubtotal,
        AddedAt:        timestamppb.Now(),
    }, nil
}
```

**Python** (25 lines of logic):
```python
def handle_add_item(command_book, command_any, state: CartState, seq: int, log):
    # GUARD
    if not state.exists():
        raise CommandRejectedError(errmsg.CART_NOT_FOUND)
    if not state.is_active():
        raise CommandRejectedError(errmsg.CART_CHECKED_OUT)

    cmd = domains.AddItem()
    command_any.Unpack(cmd)

    # VALIDATE
    if not cmd.product_id:
        raise CommandRejectedError(errmsg.PRODUCT_ID_REQUIRED)
    if cmd.quantity <= 0:
        raise CommandRejectedError(errmsg.QUANTITY_POSITIVE)

    # COMPUTE + EMIT
    new_subtotal = state.subtotal_cents + (cmd.quantity * cmd.unit_price_cents)
    event = domains.ItemAdded(
        product_id=cmd.product_id,
        name=cmd.name,
        quantity=cmd.quantity,
        unit_price_cents=cmd.unit_price_cents,
        new_subtotal=new_subtotal,
        added_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    # ... return EventBook with event
```

**Rust** (similar structure, ~40 lines with protobuf encoding)

### Why This is AI-Friendly

LLMs excel at [pattern matching from examples][1] and [pure transformations][2]. They struggle with [distributed system edge cases][3], [concurrency reasoning][4], and [infrastructure configuration][5]. Angzarr puts all the hard stuff in a layer LLMs don't touch.

[1]: https://arxiv.org/html/2509.13758v1 "A Study on Thinking Patterns of Large Reasoning Models in Code Generation (2025) — 'traditional LLMs primarily rely on pattern matching'"
[2]: https://arxiv.org/html/2601.02060 "Perish or Flourish? Evaluating LLMs for Code Generation in Functional Programming (2025)"
[3]: https://arxiv.org/html/2511.04355v1 "Where Do LLMs Still Struggle? An In-Depth Analysis of Code Generation Benchmarks (2025)"
[4]: https://arxiv.org/html/2501.14326v1 "Assessing LLMs in Comprehending and Verifying Concurrent Programs across Memory Models (2025) — 'prone to incomplete and inconsistent analysis'"
[5]: https://arxiv.org/html/2512.14792 "IaC Generation with LLMs: Error Taxonomy and Configuration Knowledge Injection (2024) — 19-27% success rate on Terraform vs 80%+ on general code"

**Prompt to generate a new handler:**

> "Write a `RemoveItem` handler for the cart aggregate. It should:
> - Reject if cart doesn't exist
> - Reject if cart is checked out
> - Reject if item not in cart
> - Emit `ItemRemoved` event with product_id and new_subtotal
>
> Follow the same pattern as `AddItem`."

An LLM produces correct code because:
- **Clear contract**: Input and output types are schema-defined
- **Examples to follow**: Every handler has identical structure
- **No hidden state**: Functions are pure
- **Testable assertion**: Given state X and command Y, expect event Z

### The Testing Story

Unit tests require no infrastructure mocking:

```python
def test_add_item_to_nonexistent_cart_rejected():
    state = CartState()  # empty, doesn't exist
    cmd = AddItem(product_id="sku-1", quantity=1, unit_price_cents=999)

    with pytest.raises(CommandRejectedError) as exc:
        handle_add_item(mock_book, cmd, state, 1, log)

    assert exc.value.args[0] == errmsg.CART_NOT_FOUND
```

```go
func TestAddItemToNonexistentCart(t *testing.T) {
    state := &CartState{} // empty
    _, err := logic.HandleAddItem(state, "sku-1", "Widget", 1, 999)

    assert.Equal(t, ErrMsgCartNotFound, err.Error())
}
```

No database setup. No container spinning. No network mocking. Pure function in, assertion out.

### The Staffing Model

```
┌─────────────────────────────────────────────────────────┐
│                    SENIOR / DEVOPS                       │
│  - Schema design (protobuf)                             │
│  - Saga orchestration                                   │
│  - Infrastructure (helm, k8s, messaging)                │
│  - Cross-cutting concerns                               │
└─────────────────────────────────────────────────────────┘
                           │
                           │ defines contracts
                           ▼
┌─────────────────────────────────────────────────────────┐
│               JUNIOR DEVS / AI AGENTS                    │
│                                                          │
│   handle_create_cart()    handle_add_item()             │
│   handle_remove_item()    handle_checkout()             │
│   handle_apply_coupon()   handle_clear_cart()           │
│                                                          │
│   (pure functions, ~30 lines each, unit testable)       │
└─────────────────────────────────────────────────────────┘
```

The senior defines the **what** (schemas, aggregates, events). Juniors and AIs implement the **how** (business rules). Neither touches the **where** (infrastructure) during normal development.

## Architecture

client logic lives in external services called via gRPC. Angzarr handles:
- **EventStore**: Persist and query events (SQLite, PostgreSQL, Redis; see [storage backends](src/storage/README.md))
- **SnapshotStore**: Optimize replay with snapshots
- **Upcaster**: Transform old event versions on read (schema evolution)
- **EventBus**: Distribute events to projectors/sagas
- **CommandHandler**: Orchestrate command processing
- **ProjectorCoordinator**: Route events to read model builders
- **SagaCoordinator**: Route events to cross-aggregate workflows
- **EventStream**: Stream filtered events to subscribers by correlation ID
- **CommandGateway**: Forward commands and stream back resulting events

### Binaries

Angzarr provides seven binaries in two categories:

**Sidecars** (run alongside your client logic in the same pod):

| Binary | Purpose |
|--------|---------|
| `angzarr-aggregate` | Command handling, event persistence, snapshot management |
| `angzarr-projector` | AMQP subscription, event routing to projector services |
| `angzarr-saga` | AMQP subscription, event routing to saga services |
| `angzarr-process-manager` | Multi-domain event subscription, stateful workflow coordination |

**Infrastructure** (central services, not sidecars):

| Binary | Purpose |
|--------|---------|
| `angzarr-gateway` | Client entry point, command routing, event streaming to clients |
| `angzarr-stream` | Correlation-based event filtering (a projector — runs with `angzarr-projector` sidecar, but the client logic is provided by the framework rather than by the client) |
| `angzarr-standalone` | Local development orchestrator — spawns all sidecars and client logic processes in a single binary, replacing Kubernetes with SQLite + Unix domain sockets |

## Documentation

**📚 [Full Documentation](https://benjaminabbitt.github.io/angzarr/)** — Docusaurus wiki with multi-language examples

Quick links:
- **[Technical Pitch](PITCH.md)** — Full architectural overview (standalone document)
- **[Getting Started](https://benjaminabbitt.github.io/angzarr/getting-started)** — Prerequisites, installation, first domain
- **[Architecture](https://benjaminabbitt.github.io/angzarr/architecture)** — Core concepts and patterns
- **[Testing](https://benjaminabbitt.github.io/angzarr/operations/testing)** — Three-level testing strategy

Components:
- [Aggregates](https://benjaminabbitt.github.io/angzarr/components/aggregate) — Command handlers
- [Sagas](https://benjaminabbitt.github.io/angzarr/components/saga) — Cross-domain coordination
- [Projectors](https://benjaminabbitt.github.io/angzarr/components/projector) — Read model builders
- [Process Managers](https://benjaminabbitt.github.io/angzarr/components/process-manager) — Stateful orchestration

## Quick Start

```bash
git clone https://github.com/benjaminabbitt/angzarr
cd angzarr
just build && just test
```

For full setup including Kubernetes, standalone mode, and your first domain, see [Getting Started](https://benjaminabbitt.github.io/angzarr/getting-started).

## License

AGPL-3.0 (GNU Affero General Public License v3)
