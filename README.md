# ⍼ Angzarr

A polyglot CQRS/Event Sourcing framework in Rust.

## What It Is

Angzarr handles the infrastructure complexity of event-sourced systems—event persistence, snapshot optimization, saga coordination, projection management—so you can focus on business logic. Your domain code runs as external gRPC services in any language; the framework handles everything else.

**[Full Documentation →](https://angzarr.io/)**

## Supported Languages

Client libraries are provided for the top TIOBE languages:

| Language | Client | Examples |
|----------|--------|----------|
| C++ | [client/cpp](client/cpp/) | [examples/cpp](examples/cpp/) |
| C# | [client/csharp](client/csharp/) | [examples/csharp](examples/csharp/) |
| Go | [client/go](client/go/) | [examples/go](examples/go/) |
| Java | [client/java](client/java/) | [examples/java](examples/java/) |
| Python | [client/python](client/python/) | [examples/python](examples/python/) |
| Rust | [client/rust](client/rust/) | [examples/rust](examples/rust/) |

**Client libraries are optional.** Any language with gRPC support can integrate directly using the [proto definitions](proto/)—the libraries just reduce boilerplate.

## Quick Start

```bash
git clone https://github.com/benjaminabbitt/angzarr
cd angzarr
just build && just test
```

See [Getting Started](https://angzarr.io/getting-started) for full setup including Kubernetes and standalone mode.

## Documentation

- **[Introduction](https://angzarr.io/)** — Problem statement, when Angzarr fits
- **[Architecture](https://angzarr.io/architecture)** — Core concepts, binaries, data flow
- **[Getting Started](https://angzarr.io/getting-started)** — Installation, first domain
- **[Components](https://angzarr.io/components)** — Aggregates, sagas, projectors, process managers
- **[Client SDKs](https://angzarr.io/sdks)** — Language-specific client libraries
- **[Technical Pitch](https://angzarr.io/pitch)** — Detailed rationale and architecture

## License

AGPL-3.0 (GNU Affero General Public License v3)
