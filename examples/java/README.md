# Java Examples

Java implementations of Angzarr bounded contexts using gRPC and JUnit 5.

## Support Status

**Best-Effort Only** - This implementation is provided as a reference. It may have incomplete test coverage, missing features, or lag behind the primary language implementations (Go, Python, Rust). Community contributions welcome.

## Structure

```
java/
├── customer/         # Customer aggregate
├── transaction/      # Transaction aggregate
├── saga-loyalty/     # Loyalty saga
├── projector-log-customer/
├── projector-log-transaction/
├── projector-receipt/
└── integration-tests/ # End-to-end Cucumber tests
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files define scenarios that are executed by [Cucumber-JVM](https://cucumber.io/docs/installation/java/).

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
cd customer && ./gradlew test
```

### Integration Tests (Cucumber)

```bash
cd integration-tests && ./gradlew test
```

## Building

```bash
cd customer && ./gradlew build
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [Cucumber-JVM](https://cucumber.io/docs/installation/java/) | BDD testing | [Docs](https://cucumber.io/docs/cucumber/) |
| [grpc-java](https://github.com/grpc/grpc-java) | gRPC framework | [Docs](https://grpc.io/docs/languages/java/) |
| [JUnit 5](https://junit.org/junit5/) | Test framework | [Docs](https://junit.org/junit5/docs/current/user-guide/) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
