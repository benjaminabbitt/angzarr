---
sidebar_position: 3
---

# Introduction to Angzarr

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

**⍼ Angzarr** is a polyglot framework for building event-sourced systems. You write business logic in any language with [gRPC support](https://grpc.io/docs/languages/)—the framework handles event persistence, saga coordination, projection management, and all the infrastructure complexity that typically derails CQRS/ES projects.

The symbol ⍼ (U+237C, "angzarr") has existed in Unicode since 2002 without a defined purpose. The right angle represents the origin point—your event store. The zigzag arrow represents events cascading through your system. We gave it meaning.

:::caution Project Status

This codebase uses **AI-generated code under human supervision**. The implementation has been reviewed, but with an emphasis on velocity over thoroughness.

This is not AI slop. The gRPC/protobuf definitions were hand-written and have evolved with heavy oversight. The architecture and code derive from an earlier human-written Go codebase—ported to Rust and used as the starting point—when the author switched for performance, binary size, and cleaner deployment mechanisms. Polyglot support has always been a core design goal—world domination requires meeting developers where they are. Architecturally, it's sound—the framework design reflects years of thinking about CQRS/ES patterns.

**Approaching production-ready, but not there yet.** The code is stabilizing and mostly working, but plan for additional testing and hardening before production deployment.

Contributors are welcome. There are a few rough edges, but mostly the project needs code review, test coverage expansion, and developers willing to build on it and report what breaks. If you're interested in CQRS/ES tooling and want to be an early adopter, jump in.

:::

---

## What Angzarr Provides

Angzarr inverts the typical framework relationship. Rather than providing libraries that applications import, Angzarr provides infrastructure that applications connect to via gRPC.

**Your data model lives in `.proto` files, not code.** Commands, events, and state are defined as Protocol Buffer messages—language-neutral, versionable, and shared across all implementations. This is what enables true polyglot support: the same event stream can be produced by a Rust aggregate and consumed by a Python projector.

| You Define | You Implement | We Handle |
|------------|---------------|-----------|
| Commands in `.proto` | Aggregate handlers | Event persistence |
| Events in `.proto` | Projector handlers | Optimistic concurrency |
| State in `.proto` | Saga handlers | Snapshot management |
| | | Event upcasting |
| | | Event distribution |
| | | Saga coordination |
| | | Schema evolution |

Your business logic receives commands with full event history and emits events. No database connections. No message bus configuration. No retry logic. Pure domain logic.

---

## Architecture Preview

⍼ Angzarr stores aggregate history as an **EventBook**—the complete event stream for a single aggregate root: its identity (the Cover), an optional Snapshot for efficient replay, and ordered EventPages representing domain events.

```mermaid
flowchart LR
    subgraph Client
        GW[Gateway]
    end

    subgraph AggPod[Domain A - Aggregate]
        AGG_COORD[⍼ Sidecar]
        AGG[Your Aggregate]
        UPC[Your Upcaster]
        AGG_COORD <--> AGG
        AGG_COORD <--> UPC
    end

    subgraph Infra[Infrastructure]
        ES[(Event Store)]
        BUS[Message Bus]
    end

    subgraph Consumers[Event Consumers]
        subgraph SagaPod[Saga Pod]
            SAGA_COORD[⍼ Sidecar]
            SAGA[Your Saga]
            SAGA_COORD <--> SAGA
        end

        subgraph PrjPod[Projector Pod]
            PRJ_COORD[⍼ Sidecar]
            PRJ[Your Projector]
            PRJ_DB[(Read Model)]
            PRJ_COORD <--> PRJ
            PRJ --> PRJ_DB
        end
    end

    subgraph AggPod2[Domain B - Aggregate]
        AGG_COORD2[⍼ Sidecar]
        AGG2[Another Aggregate]
        AGG_COORD2 <--> AGG2
    end

    GW -->|cmd| AGG_COORD
    AGG_COORD <--> ES
    AGG_COORD --> BUS
    BUS --> SAGA_COORD
    BUS --> PRJ_COORD
    SAGA_COORD -->|cmd| AGG_COORD2
    AGG_COORD2 <--> ES

    style AggPod2 stroke-dasharray: 5 5
    style AGG_COORD2 stroke-dasharray: 5 5
    style AGG2 stroke-dasharray: 5 5
```

*The dashed Domain B represents any additional domain(s)—sagas bridge events from one domain to commands in another. Real systems have multiple domains, each with its own aggregate.*

Each component type runs in its own pod with an ⍼ Angzarr sidecar. Your code handles business logic; the sidecar handles persistence, messaging, and coordination.

---

## Language Support

**Any language with gRPC support works.** Your business logic communicates with ⍼ Angzarr coordinators via gRPC—the framework doesn't care what's behind the endpoint. If your language appears on the [gRPC supported languages matrix](https://grpc.io/docs/languages/), you can use it with Angzarr. This includes C#, C++, Dart, Go, Java, Kotlin, Node.js, Objective-C, PHP, Python, Ruby, Rust, and more.

**Client libraries are optional and minimal.** For six languages (the top TIOBE languages), we provide thin client libraries that reduce boilerplate—protobuf packing/unpacking, state reconstruction, router registration. These libraries are intentionally kept lightweight; the real contract is just gRPC + protobuf. You can always work directly with the proto bindings if you prefer:

| Language | Client Library | Example |
|----------|----------------|---------|
| Python | `angzarr-client` | [examples/python/](https://github.com/benjaminabbitt/angzarr/tree/main/examples/python) |
| Go | `github.com/benjaminabbitt/angzarr/client` | [examples/go/](https://github.com/benjaminabbitt/angzarr/tree/main/examples/go) |
| Rust | `angzarr-client` | [examples/rust/](https://github.com/benjaminabbitt/angzarr/tree/main/examples/rust) |
| Java | `dev.angzarr:client` | [examples/java/](https://github.com/benjaminabbitt/angzarr/tree/main/examples/java) |
| C# | `Angzarr.Client` | [examples/csharp/](https://github.com/benjaminabbitt/angzarr/tree/main/examples/csharp) |
| C++ | header-only | [examples/cpp/](https://github.com/benjaminabbitt/angzarr/tree/main/examples/cpp) |

All six implementations share the same Gherkin specifications, ensuring identical behavior across languages.

---

## Quick Example

The same handler across all six languages. Each follows the **guard → validate → compute** pattern:

<Tabs>
<TabItem value="python" label="Python" default>

```python
# examples/python/player/agg/handlers/commands.py
@command_handler(player_proto.DepositFunds)
def handle_deposit_funds(
    cmd: player_proto.DepositFunds, state: PlayerState, seq: int
) -> player_proto.FundsDeposited:
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")

    # Validate
    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")

    # Compute
    new_balance = state.bankroll + amount
    return player_proto.FundsDeposited(
        amount=cmd.amount,
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        deposited_at=now(),
    )
```

[View full source →](https://github.com/benjaminabbitt/angzarr/blob/main/examples/python/player/agg/handlers/commands.py)

</TabItem>
<TabItem value="rust" label="Rust">

```rust
// examples/rust/player/agg/src/handlers/deposit.rs
fn guard(state: &PlayerState) -> CommandResult<()> {
    if !state.exists() {
        return Err(CommandRejectedError::new("Player does not exist"));
    }
    Ok(())
}

fn validate(cmd: &DepositFunds) -> CommandResult<i64> {
    let amount = cmd.amount.as_ref().map(|c| c.amount).unwrap_or(0);
    if amount <= 0 {
        return Err(CommandRejectedError::new("amount must be positive"));
    }
    Ok(amount)
}

fn compute(cmd: &DepositFunds, state: &PlayerState, amount: i64) -> FundsDeposited {
    let new_balance = state.bankroll + amount;
    FundsDeposited {
        amount: cmd.amount.clone(),
        new_balance: Some(Currency { amount: new_balance, currency_code: "CHIPS".into() }),
        deposited_at: Some(angzarr_client::now()),
    }
}
```

[View full source →](https://github.com/benjaminabbitt/angzarr/blob/main/examples/rust/player/agg/src/handlers/deposit.rs)

</TabItem>
<TabItem value="go" label="Go">

```go
// examples/go/player/agg/handlers/deposit.go
func guardDepositFunds(state PlayerState) error {
    if !state.Exists() {
        return angzarr.NewCommandRejectedError("Player does not exist")
    }
    return nil
}

func validateDepositFunds(cmd *examples.DepositFunds) (int64, error) {
    amount := int64(0)
    if cmd.Amount != nil {
        amount = cmd.Amount.Amount
    }
    if amount <= 0 {
        return 0, angzarr.NewCommandRejectedError("amount must be positive")
    }
    return amount, nil
}

func computeFundsDeposited(cmd *examples.DepositFunds, state PlayerState, amount int64) *examples.FundsDeposited {
    newBalance := state.Bankroll + amount
    return &examples.FundsDeposited{
        Amount:     cmd.Amount,
        NewBalance: &examples.Currency{Amount: newBalance, CurrencyCode: "CHIPS"},
    }
}
```

[View full source →](https://github.com/benjaminabbitt/angzarr/blob/main/examples/go/player/agg/handlers/deposit.go)

</TabItem>
<TabItem value="java" label="Java">

```java
// examples/java/player/agg/src/main/java/.../handlers/DepositHandler.java
public static FundsDeposited handle(DepositFunds cmd, PlayerState state) {
    // Guard
    if (!state.exists()) {
        throw Errors.CommandRejectedError.preconditionFailed("Player does not exist");
    }

    // Validate
    long amount = cmd.hasAmount() ? cmd.getAmount().getAmount() : 0;
    if (amount <= 0) {
        throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
    }

    // Compute
    long newBalance = state.getBankroll() + amount;
    return FundsDeposited.newBuilder()
        .setAmount(cmd.getAmount())
        .setNewBalance(Currency.newBuilder().setAmount(newBalance).setCurrencyCode("CHIPS"))
        .build();
}
```

[View full source →](https://github.com/benjaminabbitt/angzarr/blob/main/examples/java/player/agg/src/main/java/dev/angzarr/examples/player/handlers/DepositHandler.java)

</TabItem>
<TabItem value="csharp" label="C#">

```csharp
// examples/csharp/Player/Agg/Handlers/DepositHandler.cs
public static FundsDeposited Handle(DepositFunds cmd, PlayerState state)
{
    // Guard
    if (!state.Exists)
        throw CommandRejectedError.PreconditionFailed("Player does not exist");

    // Validate
    var amount = cmd.Amount?.Amount ?? 0;
    if (amount <= 0)
        throw CommandRejectedError.InvalidArgument("amount must be positive");

    // Compute
    var newBalance = state.Bankroll + amount;
    return new FundsDeposited
    {
        Amount = cmd.Amount,
        NewBalance = new Currency { Amount = newBalance, CurrencyCode = "CHIPS" },
    };
}
```

[View full source →](https://github.com/benjaminabbitt/angzarr/blob/main/examples/csharp/Player/Agg/Handlers/DepositHandler.cs)

</TabItem>
<TabItem value="cpp" label="C++">

```cpp
// examples/cpp/player/agg/handlers/deposit_handler.cpp
examples::FundsDeposited handle_deposit(const examples::DepositFunds& cmd, const PlayerState& state) {
    // Guard
    if (!state.exists()) {
        throw angzarr::CommandRejectedError::precondition_failed("Player does not exist");
    }

    // Validate
    int64_t amount = cmd.has_amount() ? cmd.amount().amount() : 0;
    if (amount <= 0) {
        throw angzarr::CommandRejectedError::invalid_argument("amount must be positive");
    }

    // Compute
    int64_t new_balance = state.bankroll + amount;
    examples::FundsDeposited event;
    event.mutable_amount()->CopyFrom(cmd.amount());
    event.mutable_new_balance()->set_amount(new_balance);
    event.mutable_new_balance()->set_currency_code("CHIPS");
    return event;
}
```

[View full source →](https://github.com/benjaminabbitt/angzarr/blob/main/examples/cpp/player/agg/handlers/deposit_handler.cpp)

</TabItem>
</Tabs>

**No database code. No message bus code. Just business logic.**

---

## For Decision Makers

If you're evaluating Angzarr for your organization:

- **[Technical Pitch](/pitch)** — Complete architectural pitch with detailed rationale
- **[Architecture](./architecture)** — Core concepts: data model, coordinators, sync modes
- **[Why Poker](./examples/why-poker)** — Why our example domain exercises every pattern

---

## For Developers

Ready to build:

- **[Getting Started](./getting-started)** — Prerequisites, installation, first aggregate
- **[Components](./components/aggregate)** — Aggregates, sagas, projectors, process managers
- **[Examples](./examples/aggregates)** — Code samples in all six languages

---

## Next Steps

1. **Understand the patterns** — [CQRS & Event Sourcing Explained](./patterns-explained)
2. **See the architecture** — [Architecture](./architecture)
3. **Get hands-on** — [Getting Started](./getting-started)
