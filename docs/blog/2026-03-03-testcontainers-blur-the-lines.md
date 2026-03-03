---
slug: testcontainers-blur-the-lines
title: "Testcontainers Blur the Lines Between Unit and Integration Tests"
authors: [angzarr]
tags: [testing, testcontainers, docker, rust, patterns]
keywords: [testcontainers, integration testing, unit testing, docker, contract testing, rust, testing pyramid]
---

The old unit/integration distinction assumed "integration" meant "slow, fragile, needs environment setup." Testcontainers changed the economics.

<!-- truncate -->

## The Traditional Divide

We used to draw a hard line between unit tests and integration tests:

- **Unit tests**: Fast, no external dependencies, run anywhere, colocate with code
- **Integration tests**: Slow, need databases/queues/services, run in CI, separate directory

This separation made sense when "integration test" meant "spin up a full environment." You wouldn't colocate tests that require PostgreSQL next to your repository implementation; they'd fail on every developer's machine without the right setup.

## Testcontainers Changed This

[Testcontainers](https://testcontainers.com/) (in Rust: [testcontainers-rs](https://docs.rs/testcontainers/)) spins up real infrastructure in Docker containers, on demand, per test.

```rust
#[test]
fn test_event_store_persists_events() {
    let container = PostgresContainer::new();
    let pool = connect_to(&container);
    let store = PostgresEventStore::new(pool);

    store.append("order-123", vec![event]).unwrap();
    let events = store.read("order-123").unwrap();

    assert_eq!(events.len(), 1);
}
```

This test spins up a real PostgreSQL instance in [Docker](https://www.docker.com/), runs the test against it, and tears it down. No shared database. No environment configuration. No "works on my machine." The container is ephemeral, isolated, and automatic.

## Contract Tests Fit Here

[Contract tests](https://martinfowler.com/bliki/ContractTest.html) verify that an implementation correctly fulfills its interface. Tests that verify trait implementations (`EventStore`, `SnapshotStore`, `MessageBus`) are contract tests—not "integration tests" in the traditional sense.

These tests *should* live near the implementation:

```
src/
├── storage/
│   ├── postgres.rs              # PostgresEventStore implementation
│   ├── postgres.test.rs         # Contract tests against real Postgres
│   ├── sqlite.rs
│   └── sqlite.test.rs
```

The "real database" aspect doesn't change where the test belongs. It's still testing one module's behavior. It's still colocated. It just happens to need a container.

## The New Distinction: Scope, Not Speed

The old unit/integration split was about *how* tests run. The better distinction is *what* they test.

| Test Type | What It Tests | Where It Lives |
|-----------|--------------|----------------|
| Unit | Pure logic, no dependencies | Adjacent `.test` file |
| Contract | Single implementation against its interface | Adjacent `.test` file (with testcontainers) |
| Integration | Multiple components interacting | `tests/` directory |
| End-to-end | Full system behavior | Separate test project |

Contract tests with testcontainers are closer to unit tests than integration tests. They test one thing. They're fast enough to run frequently. They should be colocated.

See Martin Fowler's [Practical Test Pyramid](https://martinfowler.com/articles/practical-test-pyramid.html) for more on scope-based test categorization.

## The CI Consideration

Yes, testcontainer tests are slower than pure unit tests. On my machine, a PostgreSQL container adds ~2 seconds of startup. That's too slow for "run on every save" but fine for "run before commit."

We handle this with test categories:

```rust
#[test]
fn test_pure_logic() { /* runs always */ }

#[test]
#[cfg_attr(not(feature = "testcontainers"), ignore)]
fn test_postgres_storage() { /* runs with --features testcontainers */ }
```

Local development runs the fast tests continuously. [Pre-commit hooks](https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks) ([we like Lefthook](https://github.com/evilmartians/lefthook)) and CI run everything. The slower tests are still colocated; they're just conditionally executed.

## Mocks Are for Boundaries, Not Implementations

This shift changed how I think about mocking. Previously, I'd mock the database to test repository logic. Now I test the repository against a real database (via testcontainers) and reserve mocks for:

- External services I don't control (third-party APIs)
- Failure injection (simulate network errors)

If I *can* test against the real thing cheaply, I should. Testcontainers made "the real thing" cheap.

## The Takeaway

The unit/integration distinction was always about economics: unit tests were cheap, integration tests were expensive. Testcontainers collapsed that cost difference for many scenarios.

When the economics change, the categories should too. Contract tests against real infrastructure aren't integration tests just because they touch a database. They're colocatable, fast-enough, single-purpose tests that happen to need Docker.

Organize by what you're testing, not by what tools you need to test it.
