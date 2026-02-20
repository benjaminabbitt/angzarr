---
title: C++ SDK
sidebar_label: C++
---

# angzarr-client-cpp

C++ client library for Angzarr event-sourcing services.

:::tip Unified Documentation
For cross-language API reference with side-by-side comparisons, see the [SDK Documentation](/sdks).
:::

## Requirements

- C++20 or later
- CMake 3.20+
- gRPC
- Protobuf
- Abseil

## Installation

### Using CMake FetchContent

```cmake
include(FetchContent)
FetchContent_Declare(
    angzarr-client
    GIT_REPOSITORY https://github.com/benjaminabbitt/angzarr.git
    GIT_TAG main
    SOURCE_SUBDIR client/cpp
)
FetchContent_MakeAvailable(angzarr-client)

target_link_libraries(your_target PRIVATE angzarr-client)
```

### Building from Source

```bash
cd client/cpp
mkdir build && cd build
cmake ..
cmake --build .
```

## Usage

### Sending Commands

```cpp
#include <angzarr/client.hpp>
#include <angzarr/builder.hpp>

int main() {
    // Connect to aggregate coordinator
    auto client = angzarr::DomainClient::connect("localhost:1310");

    // Build and send a command to create a new aggregate
    auto response = angzarr::CommandBuilder::create_new(client->aggregate(), "orders")
        .with_correlation_id("order-123")
        .with_command("type.googleapis.com/examples.CreateOrder", create_order_cmd)
        .execute();

    // Get the new aggregate root ID from response
    auto root_id = angzarr::root_uuid(response.events());
    std::cout << "Created order: " << root_id << std::endl;

    return 0;
}
```

### Querying Events

```cpp
#include <angzarr/client.hpp>
#include <angzarr/builder.hpp>

// Connect to query service
auto client = angzarr::DomainClient::connect("localhost:1310");

// Query events for an aggregate
angzarr::Query query;
query.mutable_cover()->set_domain("orders");
// Set root UUID...

auto events = client->query()->get_event_book(query);

// Iterate over events
for (const auto& page : events.pages()) {
    std::cout << "Event " << angzarr::sequence_num(page)
              << ": " << angzarr::type_name_from_url(page.event().type_url())
              << std::endl;
}
```

### Using Environment Variables

```cpp
// Connect using environment variable with fallback
auto client = angzarr::DomainClient::from_env("ANGZARR_ENDPOINT", "localhost:1310");
```

### Error Handling

```cpp
#include <angzarr/client.hpp>
#include <angzarr/errors.hpp>

try {
    auto response = client->aggregate()->handle(command);
} catch (const angzarr::ClientError& e) {
    if (e.is_not_found()) {
        // Aggregate doesn't exist
    } else if (e.is_precondition_failed()) {
        // Sequence mismatch (optimistic locking failure)
    } else if (e.is_invalid_argument()) {
        // Invalid command arguments
    } else if (e.is_connection_error()) {
        // Network/transport error
    }
}
```

### Speculative Execution

Test commands without persisting to the event store:

```cpp
#include <angzarr/client.hpp>

// AggregateClient supports speculative execution directly
auto client = angzarr::AggregateClient::connect("localhost:1310");

// Build speculative request with temporal state
angzarr::SpeculateAggregateRequest request;
*request.mutable_command() = command_book;
for (const auto& event : prior_events) {
    *request.add_events() = event;
}

// Execute without persistence
auto response = client->handle_sync_speculative(request);

// Inspect projected events
for (const auto& page : response.events().pages()) {
    std::cout << "Would produce: " << page.event().type_url() << std::endl;
}
```

### Compensation Context

Extract rejection details when saga/PM commands fail:

```cpp
#include <angzarr/compensation.hpp>

// In your saga or process manager rejection handler
auto context = angzarr::CompensationContext::from_notification(notification);

std::cout << "Issuer: " << context.issuer_name()
          << " (" << context.issuer_type() << ")" << std::endl;
std::cout << "Rejection reason: " << context.rejection_reason() << std::endl;
std::cout << "Failed command type: " << context.rejected_command_type() << std::endl;
std::cout << "Source event sequence: " << context.source_event_sequence() << std::endl;
```

## Client Types

| Client | Purpose |
|--------|---------|
| `QueryClient` | Query events from aggregates |
| `AggregateClient` | Send commands to aggregates |
| `DomainClient` | Combined query + aggregate for a domain |

## Error Types

| Error | Description | Introspection |
|-------|-------------|---------------|
| `ClientError` | Base class for all errors | All methods return `false` |
| `CommandRejectedError` | Business logic rejection | `is_precondition_failed()` |
| `GrpcError` | gRPC transport failure | Based on status code |
| `ConnectionError` | Connection failure | `is_connection_error()` |
| `TransportError` | Transport-level failure | `is_connection_error()` |
| `InvalidArgumentError` | Invalid input | `is_invalid_argument()` |

## Helper Functions

```cpp
#include <angzarr/helpers.hpp>

// UUID conversion
auto uuid = angzarr::proto_to_uuid(proto_uuid);
auto proto_uuid = angzarr::uuid_to_proto(uuid);

// Type URL helpers
auto type_url = angzarr::type_url("examples.CreateOrder");  // "type.googleapis.com/examples.CreateOrder"
auto type_name = angzarr::type_name_from_url(type_url);     // "CreateOrder"

// Cover accessors
auto domain = angzarr::domain(event_book);
auto correlation_id = angzarr::correlation_id(event_book);
auto root_uuid = angzarr::root_uuid(event_book);

// Sequence helpers
auto next_seq = angzarr::next_sequence(event_book);
```

## Building with vcpkg

For containerized builds using vcpkg:

```dockerfile
FROM mcr.microsoft.com/vcpkg:latest
RUN vcpkg install grpc protobuf abseil
COPY . /app
WORKDIR /app/client/cpp/build
RUN cmake -DCMAKE_TOOLCHAIN_FILE=/vcpkg/scripts/buildsystems/vcpkg.cmake ..
RUN cmake --build .
```

## License

AGPL-3.0-only
