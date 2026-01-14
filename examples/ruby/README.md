# Ruby Examples

Ruby implementations of Angzarr bounded contexts using gRPC and RSpec.

## Support Status

**Best-Effort Only** - This implementation is provided as a reference. It may have incomplete test coverage, missing features, or lag behind the primary language implementations (Go, Python, Rust). Community contributions welcome.

## Structure

```
ruby/
├── customer/         # Customer aggregate
├── transaction/      # Transaction aggregate
├── saga-loyalty/     # Loyalty saga
├── projector-log-customer/
├── projector-log-transaction/
└── projector-receipt/
```

## Acceptance Testing with Gherkin

Tests use [Gherkin](https://cucumber.io/docs/gherkin/) syntax - a human-readable language for describing software behavior using Given/When/Then steps. Feature files define scenarios that are executed by [Cucumber-Ruby](https://github.com/cucumber/cucumber-ruby).

Example:
```gherkin
Scenario: Create a new customer
  Given no prior events for the aggregate
  When I handle a CreateCustomer command with name "Alice" and email "alice@example.com"
  Then the result is a CustomerCreated event
```

## Running Tests

### Unit Tests (RSpec)

```bash
cd customer && bundle exec rspec

# Via just
just test-ruby
```

### Acceptance Tests (Cucumber)

```bash
cd customer && bundle exec cucumber

# Via just
just acceptance-ruby
```

## Building

```bash
cd customer && bundle install
```

## Port Configuration

Services bind to `GRPC_PORT` environment variable (default: 50051). For concurrent deployments, set unique ports per instance.

## Dependencies

| Library | Purpose | Documentation |
|---------|---------|---------------|
| [Cucumber-Ruby](https://github.com/cucumber/cucumber-ruby) | BDD testing | [Docs](https://cucumber.io/docs/installation/ruby/) |
| [grpc](https://rubygems.org/gems/grpc) | gRPC framework | [Docs](https://grpc.io/docs/languages/ruby/) |
| [RSpec](https://rspec.info/) | Test framework | [Docs](https://rspec.info/documentation/) |

## References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)
