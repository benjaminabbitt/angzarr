# C# Examples

C# implementations of Angzarr bounded contexts using gRPC and xUnit.

## Support Status

**Best-Effort Only** - This implementation is provided as a reference. It may have incomplete test coverage, missing features, or lag behind the primary language implementations (Go, Python, Rust). Community contributions welcome.

**Note:** Test coverage is currently minimal. Only Customer has unit tests.

## Structure

```
csharp/
├── Customer/              # Customer aggregate
├── Customer.Tests/        # Customer unit tests
├── Transaction/           # Transaction aggregate
├── SagaLoyalty/          # Loyalty saga
├── ProjectorLogCustomer/
├── ProjectorLogTransaction/
├── ProjectorReceipt/
└── Integration.Tests/     # Integration tests (WIP)
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files define scenarios that are executed by [SpecFlow](https://specflow.org/).

Example:
```gherkin
Scenario: Create a new customer
  Given no prior events for the aggregate
  When I handle a CreateCustomer command with name "Alice" and email "alice@example.com"
  Then the result is a CustomerCreated event
```

## Running Tests

### Unit Tests

```bash
dotnet test Customer.Tests/
```

## Building

```bash
dotnet build
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [SpecFlow](https://specflow.org/) | BDD testing | [Docs](https://docs.specflow.org/) |
| [Grpc.AspNetCore](https://grpc.io/docs/languages/csharp/) | gRPC framework | [Docs](https://grpc.io/docs/languages/csharp/quickstart/) |
| [xUnit](https://xunit.net/) | Test framework | [Docs](https://xunit.net/docs/getting-started/netcore/cmdline) |
| [Moq](https://github.com/moq/moq4) | Mocking | [Docs](https://github.com/moq/moq4#readme) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
