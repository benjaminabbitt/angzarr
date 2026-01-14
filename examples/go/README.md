# Go Examples

Go implementations of Angzarr bounded contexts using gRPC and godog for BDD testing.

## Support Status

**Primary Language** - Fully supported with complete test coverage.

## Structure

```
go/
├── common/           # Shared protobuf definitions
├── customer/         # Customer aggregate
├── transaction/      # Transaction aggregate
├── saga-loyalty/     # Loyalty saga
├── projector-log-customer/
├── projector-log-transaction/
├── projector-receipt/
└── k8s/              # Language-specific K8s config
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files in `features/` define scenarios that are executed by [godog](https://github.com/cucumber/godog).

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
# All unit tests
cd customer && go test ./logic/...

# Via just
just test-go-customer
```

### Acceptance Tests (Cucumber/godog)

```bash
# Customer acceptance tests
cd customer && go test ./features/...

# Via just
just acceptance-go-customer
```

## Building

```bash
just examples-go
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance. MongoDB uses `angzarr_go` database by default.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [godog](https://github.com/cucumber/godog) | BDD/Cucumber testing | [Docs](https://github.com/cucumber/godog#readme) |
| [grpc-go](https://github.com/grpc/grpc-go) | gRPC framework | [Docs](https://grpc.io/docs/languages/go/) |
| [testify](https://github.com/stretchr/testify) | Assertions | [Docs](https://pkg.go.dev/github.com/stretchr/testify) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
