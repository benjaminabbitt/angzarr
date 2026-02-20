---
title: Python SDK
sidebar_label: Python
---

# angzarr-client

Python client library for Angzarr CQRS/ES framework.

:::tip Unified Documentation
For cross-language API reference with side-by-side comparisons, see the [SDK Documentation](/sdks).
:::

## Installation

```bash
pip install angzarr-client
```

## Client Usage

```python
from angzarr_client import DomainClient

# Connect to a domain's aggregate coordinator
client = DomainClient("localhost:1310")

# Build and execute a command
response = client.command("order", root_id) \
    .with_command("CreateOrder", create_order_msg) \
    .execute()

# Query events
events = client.query("order", root_id) \
    .get_event_book()
```

## Aggregate Implementation

Two approaches for implementing aggregates:

### 1. Rich Domain Model (Recommended)

Use `Aggregate` ABC with `@handles` decorator for OO-style aggregates:

```python
from angzarr_client import Aggregate, handles
from angzarr_client.errors import CommandRejectedError

@dataclass
class _PlayerState:
    player_id: str = ""
    bankroll: int = 0

class Player(Aggregate[_PlayerState]):
    domain = "player"  # Required

    def _create_empty_state(self) -> _PlayerState:
        return _PlayerState()

    def _apply_event(self, state: _PlayerState, event_any) -> None:
        if event_any.type_url.endswith("PlayerRegistered"):
            event = PlayerRegistered()
            event_any.Unpack(event)
            state.player_id = event.player_id

    @handles(RegisterPlayer)
    def register(self, cmd: RegisterPlayer) -> PlayerRegistered:
        if self.exists:
            raise CommandRejectedError("Player already exists")
        return PlayerRegistered(player_id=cmd.player_id, ...)

    @handles(DepositFunds)
    def deposit(self, cmd: DepositFunds) -> FundsDeposited:
        ...

    @property
    def exists(self) -> bool:
        return bool(self._get_state().player_id)
```

**Features:**
- `@handles(CommandType)` validates type hints at decoration time
- Dispatch table built automatically at class definition
- `domain` attribute required, enforced at class creation
- Abstract methods `_create_empty_state()` and `_apply_event()` enforced

**gRPC Server:**
```python
from angzarr_client import run_aggregate_server

run_aggregate_server(Player, "50303")
```

### 2. Function-Based (CommandRouter)

Use `CommandRouter` with standalone handler functions:

```python
from angzarr_client import CommandRouter
from angzarr_client.proto.angzarr import types_pb2 as types

def rebuild_state(event_book: types.EventBook) -> PlayerState:
    state = PlayerState()
    if event_book:
        for page in event_book.pages:
            apply_event(state, page.event)
    return state

def handle_register(cb, cmd_any, state, seq) -> types.EventBook:
    cmd = RegisterPlayer()
    cmd_any.Unpack(cmd)
    if state.exists:
        raise CommandRejectedError("Player already exists")
    event = PlayerRegistered(player_id=cmd.player_id, ...)
    return pack_event(event, seq)

router = CommandRouter("player", rebuild_state) \
    .on("RegisterPlayer", handle_register) \
    .on("DepositFunds", handle_deposit)
```

**gRPC Server:**
```python
from angzarr_client import run_aggregate_server

run_aggregate_server(router, "50303")
```

### Comparison

| Aspect | Rich Domain Model | Function-Based |
|--------|------------------|----------------|
| Pattern | OO, encapsulated | Procedural, explicit |
| State | Internal, lazy rebuild | External, passed in |
| Commands | Method per command | Function per command |
| Validation | `@handles` decorator | Manual type unpacking |
| Topology | Auto from `domain` + `@handles` | Auto from `CommandRouter.on()` |

## Testing Aggregates

Both patterns support unit testing without infrastructure:

```python
# Rich Domain Model
def test_register_creates_player():
    player = Player()  # Empty event book
    event = player.register(RegisterPlayer(player_id="alice"))
    assert event.player_id == "alice"
    assert player.exists

# With prior state (rehydration)
def test_deposit_increases_bankroll():
    event_book = build_event_book([PlayerRegistered(...)])
    player = Player(event_book)
    event = player.deposit(DepositFunds(amount=100))
    assert player.bankroll == 100
```

## Error Handling

```python
from angzarr_client.errors import GRPCError, ConnectionError, ClientError

try:
    response = client.aggregate.handle(command)
except GRPCError as e:
    if e.is_not_found():
        # Aggregate doesn't exist
        pass
    elif e.is_precondition_failed():
        # Sequence mismatch (optimistic locking failure)
        pass
    elif e.is_invalid_argument():
        # Invalid command arguments
        pass
except ConnectionError as e:
    # Network/transport error
    pass
```

## Speculative Execution

Test commands without persisting to the event store:

```python
from angzarr_client import SpeculativeClient
from angzarr_client.proto.angzarr import SpeculateAggregateRequest

client = SpeculativeClient.connect("localhost:1310")

# Build speculative request with temporal state
request = SpeculateAggregateRequest(
    command=command_book,
    events=prior_events
)

# Execute without persistence
response = client.aggregate(request)

# Inspect projected events
for page in response.events.pages:
    print(f"Would produce: {page.event.type_url}")

client.close()
```

## Client Types

| Client | Purpose |
|--------|---------|
| `QueryClient` | Query events from aggregates |
| `AggregateClient` | Send commands to aggregates |
| `SpeculativeClient` | Dry-run commands, test projectors/sagas |
| `DomainClient` | Combined query + aggregate for a domain |
| `Client` | Full client with all capabilities |

## Error Types

| Error | Description |
|-------|-------------|
| `ClientError` | Base class for all errors |
| `CommandRejectedError` | Business logic rejection |
| `GRPCError` | gRPC transport failure (has introspection methods) |
| `ConnectionError` | Connection failure |
| `TransportError` | Transport-level failure |
| `InvalidArgumentError` | Invalid input |
| `InvalidTimestampError` | Timestamp parse failure |

## License

AGPL-3.0 - See [LICENSE](LICENSE) for details.
