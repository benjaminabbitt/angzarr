# TypeScript Examples

TypeScript implementations of Angzarr bounded contexts using gRPC and Vitest.

## Support Status

**Best-Effort Only** - This implementation is provided as a reference. It may have incomplete test coverage, missing features, or lag behind the primary language implementations (Go, Python, Rust). Community contributions welcome.

## Structure

```
typescript/
├── customer/         # Customer aggregate
├── transaction/      # Transaction aggregate
├── saga-loyalty/     # Loyalty saga
├── projector-log-customer/
├── projector-log-transaction/
└── projector-receipt/
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files define scenarios that are executed by [Cucumber.js](https://github.com/cucumber/cucumber-js).

Example:
```gherkin
Scenario: Create a new customer
  Given no prior events for the aggregate
  When I handle a CreateCustomer command with name "Alice" and email "alice@example.com"
  Then the result is a CustomerCreated event
```

## Running Tests

### Unit Tests (Vitest)

```bash
cd customer && npm test

# Via just
just test-typescript
```

### Acceptance Tests (Cucumber)

```bash
cd customer && npm run cucumber

# Via just
just acceptance-typescript
```

## Building

```bash
cd customer && npm install && npm run build
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [Cucumber.js](https://github.com/cucumber/cucumber-js) | BDD testing | [Docs](https://cucumber.io/docs/installation/javascript/) |
| [@grpc/grpc-js](https://www.npmjs.com/package/@grpc/grpc-js) | gRPC framework | [Docs](https://grpc.io/docs/languages/node/) |
| [Vitest](https://vitest.dev/) | Test framework | [Docs](https://vitest.dev/guide/) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
