# Python Examples

Python implementations of Angzarr bounded contexts using gRPC and pytest-bdd for testing.

## Support Status

**Primary Language** - Fully supported with complete test coverage.

## Structure

```
python/
├── customer/         # Customer aggregate
├── transaction/      # Transaction aggregate
├── saga-loyalty/     # Loyalty saga
├── projector-log-customer/
├── projector-log-transaction/
└── k8s/              # Language-specific K8s config
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files in `features/` define scenarios that are executed by [pytest-bdd](https://pytest-bdd.readthedocs.io/).

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
# All unit tests for a service
cd customer && uv run pytest test_*.py

# Via just
just examples-test
```

### Acceptance Tests (pytest-bdd)

```bash
# Customer acceptance tests
cd customer && uv run pytest features/

# Via just
just acceptance-python-customer
```

## Building

```bash
just examples-python
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance. MongoDB uses `angzarr_python` database by default.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [pytest-bdd](https://pytest-bdd.readthedocs.io/) | BDD/Cucumber testing | [Docs](https://pytest-bdd.readthedocs.io/en/stable/) |
| [grpcio](https://grpc.io/docs/languages/python/) | gRPC framework | [Docs](https://grpc.io/docs/languages/python/quickstart/) |
| [pytest](https://docs.pytest.org/) | Test framework | [Docs](https://docs.pytest.org/en/stable/) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
