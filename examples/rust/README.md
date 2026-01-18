# Rust Examples

Rust implementations of Angzarr bounded contexts using tonic (gRPC) and cucumber-rs for BDD testing.

## Support Status

**Primary Language** - Fully supported with complete test coverage.

## Structure

```
rust/
├── common/           # Shared types and protobuf definitions
├── customer/         # Customer aggregate
├── transaction/      # Transaction aggregate
├── saga-loyalty/     # Loyalty saga
├── projector-log-customer/
├── projector-log-transaction/
└── k8s/              # Language-specific K8s config
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files in `tests/features/` define scenarios that are executed by [cucumber-rs](https://github.com/cucumber-rs/cucumber).

Example from `customer.feature`:
```gherkin
Scenario: Create a new customer
  Given no prior events for the aggregate
  When I handle a CreateCustomer command with name "Alice" and email "alice@example.com"
  Then the result is a CustomerCreated event
```

## Running Tests

### Unit Tests

```bash
# All tests for a service
cd customer && cargo test

# Library tests only
cd customer && cargo test --lib
```

### Acceptance Tests (cucumber-rs)

```bash
# Run Cucumber tests
cd customer && cargo test --test cucumber

# Via just
just acceptance-rust-customer
```

## Building

```bash
just examples-rust
```

## Skaffold (Fast Local Development)

Skaffold provides fast iteration by watching files and rebuilding/redeploying on change. Works with podman via a local registry.

### One-time Setup

1. Configure podman to trust the local registry (`~/.config/containers/registries.conf`):
   ```toml
   [[registry]]
   location="localhost:5001"
   insecure=true
   ```

2. Configure skaffold to disable kind's default image loading (`~/.skaffold/config`):
   ```yaml
   global:
     kind-disable-load: true
   ```

3. Create the kind cluster with local registry:
   ```bash
   just skaffold-setup
   ```

### Usage

```bash
# Dev loop - watches files, rebuilds on change
just skaffold-dev

# Build and deploy once
just skaffold-run

# Build images only
just skaffold-build

# Clean up
just skaffold-delete
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance. MongoDB uses `angzarr_rust` database by default.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [cucumber](https://github.com/cucumber-rs/cucumber) | BDD/Cucumber testing | [Docs](https://cucumber-rs.github.io/cucumber/main/) |
| [tonic](https://github.com/hyperium/tonic) | gRPC framework | [Docs](https://docs.rs/tonic/) |
| [tokio](https://tokio.rs/) | Async runtime | [Docs](https://docs.rs/tokio/) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
