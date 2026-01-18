# ⍼ Angzarr

**Schema-first CQRS/ES for polyglot business logic**

---

## What It Is

Angzarr is infrastructure that applications connect to, not libraries they import. Define your domain in Protocol Buffers. Implement business logic as gRPC services in any language. The framework handles everything else.

The symbol ⍼ ([U+237C](https://en.wikipedia.org/wiki/Angzarr)) has existed in Unicode since 2002 with no defined purpose—until now.

## The Value Proposition

| You Define | You Implement | We Handle |
|------------|---------------|-----------|
| Commands in `.proto` | gRPC `BusinessLogic` service | Event persistence |
| Events in `.proto` | gRPC `Projector` services | Optimistic concurrency |
| Read models in `.proto` | gRPC `Saga` services | Snapshot management |
| | | Event distribution |
| | | Saga coordination |
| | | Schema evolution rules |

## How It Works

```
┌─ Your Pod ─────────────────────────────────────────────────┐
│  ┌────────────────┐  localhost   ┌────────────────────┐   │
│  │ Your Aggregate │◄────gRPC────►│ Angzarr Sidecar    │   │
│  │ (Go/Python/Rust)│              │ (~8MB, distroless) │   │
│  └────────────────┘              └─────────┬──────────┘   │
└────────────────────────────────────────────┼──────────────┘
                                             │
                    ┌────────────────────────┴────────────────┐
                    ▼                                         ▼
           ┌──────────────┐                          ┌──────────────┐
           │  Event Store │                          │  Message Bus │
           │  (Postgres)  │                          │  (RabbitMQ)  │
           └──────────────┘                          └──────────────┘
```

## The Book Metaphor

- **EventBook**: Complete aggregate history (Cover + Snapshot + EventPages)
- **CommandBook**: Commands targeting an aggregate (Cover + CommandPages)
- **ContextualCommand**: EventBook + CommandBook delivered to your logic

## Multi-Language

Your business logic implements one gRPC interface:

```protobuf
service BusinessLogic {
  rpc Handle(ContextualCommand) returns (EventBook);
}
```

| Language | Status |
|----------|--------|
| Rust | Native |
| Go | Production |
| Python | Production |
| Java | Beta |
| C# | Beta |

## Infrastructure Adapters

**Event Store:** MongoDB · PostgreSQL · EventStoreDB · *Redis (untested)*

**Message Bus:** Direct gRPC · RabbitMQ · *Kafka (planned)*

## Key Capabilities

- **Schema-first** — Protobuf defines commands, events, read models
- **Optimistic concurrency** — Sequence-based conflict detection
- **Snapshot optimization** — Efficient aggregate hydration
- **Sync/async projections** — Return read models with command response
- **Saga coordination** — Cross-aggregate workflows, depth limiting
- **~8MB sidecar** — Distroless, minimal attack surface

## Quick Start

```bash
git clone https://github.com/angzarr/angzarr
cd angzarr
just kind-create  # Local K8s cluster
just deploy       # Deploy framework + examples
```

## Trade-offs

| Fits Well | Consider Alternatives |
|-----------|----------------------|
| Complex domains needing audit trails | Simple CRUD apps |
| Polyglot organizations | Browser-only (no gRPC) |
| Independent command/query scaling | Library-style preference |
| Infrastructure portability | Managed service preference |

---

**⍼ Angzarr**: Define schema. Implement logic. We handle the rest.
