# Angzarr Examples

Example implementations demonstrating the Angzarr CQRS/Event Sourcing framework across multiple languages.

## Command Runner

This project uses [just](https://github.com/casey/just) as its command runner. If you're familiar with Makefiles, `just` will feel familiar - it uses a similar syntax but is purpose-built for running commands rather than building files. Justfiles are readable even without prior experience.

```bash
# Install just
brew install just        # macOS
cargo install just       # any platform with Rust

# List available commands
just

# Run a command
just build
just test
```

Each example directory has its own `justfile` and is largely self-sufficient - you can run `just build`, `just test`, etc. from within any language directory. The only external dependency is the angzarr framework binaries/sidecars.

For full documentation on `just`, see the [main README](../README.md#about-just).

## Example Types

Each language implements 6 bounded context examples:

| Example | Type | Description |
|---------|------|-------------|
| `customer` | Aggregate | Customer lifecycle and loyalty points management |
| `transaction` | Aggregate | Transaction/order processing |
| `saga-loyalty` | Saga | Loyalty program orchestration across domains |
| `projector-log-customer` | Projector | Customer event logging |
| `projector-log-transaction` | Projector | Transaction event logging |

## Language Support

### Primary Languages (Fully Supported)

These languages have complete test coverage, active maintenance, and are recommended for production use:

| Language | Directory | Unit Tests | Acceptance Tests |
|----------|-----------|------------|------------------|
| **Go** | `go/` | `*_test.go` in `logic/` | godog in `features/` |
| **Python** | `python/` | `test_*.py` | pytest-bdd in `features/` |
| **Rust** | `rust/` | `#[cfg(test)]` modules | cucumber-rs |

### Best-Effort Languages

These languages are provided as reference implementations. They may have incomplete test coverage and are not actively maintained:

| Language | Directory | Status |
|----------|-----------|--------|
| Java | `java/` | Unit tests + integration tests |
| Kotlin | `kotlin/` | Unit tests + Cucumber |
| C# | `csharp/` | Partial unit tests |
| Ruby | `ruby/` | RSpec unit tests |
| TypeScript | `typescript/` | Vitest unit tests |

## Acceptance Testing with Gherkin

All examples share [Gherkin](https://cucumber.io/docs/gherkin/) feature files in `features/` for consistent behavior verification. Gherkin is a human-readable language for describing software behavior using Given/When/Then steps:

```gherkin
Scenario: Create a new customer
  Given no prior events for the aggregate
  When I handle a CreateCustomer command with name "Alice" and email "alice@example.com"
  Then the result is a CustomerCreated event
```

### Shared Feature Files

- `customer.feature` - Customer aggregate scenarios
- `transaction.feature` - Transaction scenarios
- `saga-loyalty.feature` - Saga orchestration scenarios
- `projector-log.feature` - Logging projector scenarios

### References

- [Gherkin Reference](https://cucumber.io/docs/gherkin/reference/)
- [Cucumber Documentation](https://cucumber.io/docs/)

## Running Tests

From the `examples/` directory:

```bash
# All primary languages (Rust, Go, Python)
just test

# Individual languages
just rust test
just go test
just python test

# Full test suite (unit + acceptance)
just full-test-rust
just full-test-go
just full-test-python

# Full stack (unit + acceptance + integration with k8s)
just full-stack-rust
just full-stack-go
just full-stack-python
```

Or from the repository root using the examples module:

```bash
just examples test
just examples full-stack-rust
```

### Language-Specific Testing

Each language directory is self-contained:

```bash
cd rust && just test
cd go && just test
cd python && just test
```

## Building

From the `examples/` directory:

```bash
# All primary languages
just build

# Individual languages
just rust build
just go build
just python setup
```

Or from the repository root:

```bash
just examples build
just examples rust build
```

## Port Configuration

Each language has a unique port range to allow concurrent deployments:

| Language | Range | Customer | Transaction | Saga | Log-Cust | Log-Trans |
|----------|-------|----------|-------------|------|----------|-----------|
| Rust | 50100s | 50100 | 50101 | 50102 | 50104 | 50105 |
| Go | 50200s | 50200 | 50201 | 50202 | 50204 | 50205 |
| Python | 50300s | 50300 | 50301 | 50302 | 50304 | 50305 |
| TypeScript | 50400s | 50400 | 50401 | 50402 | 50404 | 50405 |
| Kotlin | 50500s | 50500 | 50501 | 50502 | 50504 | 50505 |
| C# | 50600s | 50600 | 50601 | 50602 | 50604 | 50605 |
| Java | 50700s | 50700 | 50701 | 50702 | 50704 | 50705 |
| Ruby | 50800s | 50800 | 50801 | 50802 | 50804 | 50805 |

See language-specific READMEs for details.
