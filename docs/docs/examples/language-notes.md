---
sidebar_position: 5
---

# Language-Specific Notes

Notable differences across Python, Go, Rust, Java, C#, and C++ implementations.

---

## Quick Reference

| Concern | Python | Go | Rust |
|---------|--------|----|----- |
| **Error returns** | `raise CommandRejectedError()` | Return `(nil, err)` | `Result<T, Status>` |
| **State mutation** | Mutable dataclasses | Pointer receivers | `&mut self` |
| **Timestamp** | `Timestamp(seconds=...)` | `timestamppb.Now()` | `prost_types::Timestamp` |
| **Any packing** | `any.Pack(event)` | `anypb.MarshalFrom()` | `Any::from_msg()` |

| Concern | Java | C# | C++ |
|---------|------|----|-----|
| **Error returns** | `throw new CommandRejectedError()` | `throw new CommandRejectedError()` | `throw CommandRejectedError()` |
| **State mutation** | Mutable POJOs | Mutable classes | `std::shared_ptr<State>` |
| **Timestamp** | `Timestamps.fromMillis()` | `Timestamp.FromDateTime()` | `google::protobuf::Timestamp` |
| **Any packing** | `Any.pack(event)` | `Any.Pack(event)` | `event.PackTo(&any)` |

---

## Python

### Protobuf Handling

```python
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

# Packing events
event_any = Any()
event_any.Pack(event, type_url_prefix="type.googleapis.com/")

# Unpacking events
if event_any.Is(PlayerRegistered.DESCRIPTOR):
    event = PlayerRegistered()
    event_any.Unpack(event)

# Timestamps
import time
timestamp = Timestamp(seconds=int(time.time()))
```

### Gotchas

- Protobuf messages are mutable—avoid accidental mutation
- Use `Timestamp(seconds=int(...))` not datetime objects
- gRPC status codes via `grpc.StatusCode.FAILED_PRECONDITION`

---

## Go

### Protobuf Handling

```go
import (
    "google.golang.org/protobuf/types/known/anypb"
    "google.golang.org/protobuf/types/known/timestamppb"
)

// Packing events
eventAny, err := anypb.New(event)

// Unpacking events
var event PlayerRegistered
if err := eventAny.UnmarshalTo(&event); err != nil {
    // handle error
}

// Timestamps
timestamp := timestamppb.Now()
```

### Gotchas

- Return tuple `(result, error)`—never panic
- Use `status.Error(codes.X, msg)` for gRPC errors
- Protobuf `timestamppb.Now()` for timestamps

---

## Rust

### Protobuf Handling

```rust
use prost_types::{Any, Timestamp};
use std::time::SystemTime;

// Packing events
let event_any = Any::from_msg(&event)?;

// Unpacking events
let event: PlayerRegistered = event_any.to_msg()?;

// Timestamps
let timestamp = Timestamp::from(SystemTime::now());
```

### Gotchas

- `String` vs `&str`—proto fields are `String`, clone if needed
- `Option<T>` for optional proto fields (timestamps, nested messages)
- Use `Result<T, Status>` for error handling
- `prost_types::Any::from_msg()` for packing

---

## Java

### Protobuf Handling

```java
import com.google.protobuf.Any;
import com.google.protobuf.Timestamp;
import com.google.protobuf.util.Timestamps;

// Packing events
Any eventAny = Any.pack(event);

// Unpacking events
if (eventAny.is(PlayerRegistered.class)) {
    PlayerRegistered event = eventAny.unpack(PlayerRegistered.class);
}

// Timestamps
Timestamp timestamp = Timestamps.fromMillis(System.currentTimeMillis());
```

### Gotchas

- Proto messages are immutable—use builders
- Use `StatusRuntimeException` for gRPC errors
- Check `is()` before `unpack()` to avoid exceptions

---

## C#

### Protobuf Handling

```csharp
using Google.Protobuf.WellKnownTypes;

// Packing events
var eventAny = Any.Pack(evt);

// Unpacking events
if (eventAny.Is(PlayerRegistered.Descriptor))
{
    var evt = eventAny.Unpack<PlayerRegistered>();
}

// Timestamps
var timestamp = Timestamp.FromDateTime(DateTime.UtcNow);
```

### Gotchas

- Proto messages are mutable in C# (unlike Java)
- Use `RpcException` for gRPC errors
- Always use UTC for timestamps

---

## C++

### Protobuf Handling

```cpp
#include <google/protobuf/any.pb.h>
#include <google/protobuf/timestamp.pb.h>

// Packing events
google::protobuf::Any event_any;
event_any.PackFrom(event);

// Unpacking events
PlayerRegistered event;
if (event_any.Is<PlayerRegistered>()) {
    event_any.UnpackTo(&event);
}

// Timestamps
google::protobuf::Timestamp timestamp;
timestamp.set_seconds(std::time(nullptr));
```

### Gotchas

- Memory management—use smart pointers
- Check `Is<T>()` before `UnpackTo()`
- gRPC status via `grpc::Status(grpc::StatusCode::X, msg)`

---

## Error Handling Comparison

All languages use the guard/validate/compute pattern, but error propagation differs:

```
Python:  raise CommandRejectedError(errmsg.PLAYER_NOT_REGISTERED)
Go:      return nil, status.Error(codes.FailedPrecondition, errmsg.PlayerNotRegistered)
Rust:    return Err(Status::failed_precondition(errmsg::PLAYER_NOT_REGISTERED))
Java:    throw new CommandRejectedError(ErrMsg.PLAYER_NOT_REGISTERED)
C#:      throw new CommandRejectedError(ErrMsg.PlayerNotRegistered)
C++:     throw CommandRejectedError(errmsg::PLAYER_NOT_REGISTERED)
```

The error messages are identical across languages—only the propagation mechanism differs.

---

## Next Steps

- **[Aggregates](/examples/aggregates)** — Full handler examples
- **[Testing](/operations/testing)** — Cross-language Gherkin tests
