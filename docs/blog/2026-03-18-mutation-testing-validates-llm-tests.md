---
slug: mutation-testing-validates-llm-tests
title: "Mutation Testing: The Deterministic Arbiter of LLM-Generated Tests"
authors: [angzarr]
tags: [llm, testing, mutation-testing, tdd, workflow]
keywords: [llm, ai, mutation testing, cargo mutants, test quality, code coverage, tdd, deterministic verification]
---

import BlogHeader from '@site/src/components/BlogHeader';

<BlogHeader />

Last week I argued that you should [build deterministic systems with non-deterministic tools](/blog/deterministic-systems-non-deterministic-tools)—demand TDD from your LLM, get tests first, then implementation. But there's a problem with that workflow: passing tests aren't proof that tests are good.

Enter mutation testing: a deterministic tool that validates whether your tests actually test anything.

<!-- truncate -->

## The Gap in TDD

The previous post established a workflow: LLM writes tests, you review them, LLM implements, tests pass. But consider this test:

```rust
#[test]
fn test_reservation_prevents_double_booking() {
    let mut player = Player::new(500);
    player.reserve(300).unwrap();

    // This test "passes" but proves nothing
    assert!(true);
}
```

The test runs. It passes. Code coverage tools say the `reserve` function was called. Everything looks green. But the test validates nothing—you could delete the entire `reserve` implementation and this test would still pass.

An LLM optimizing for "make the tests pass" might produce exactly this kind of hollow test. Not maliciously—the test looks reasonable at a glance. It calls the right functions. It has assertions. But the assertions don't constrain the behavior.

This is where mutation testing enters.

## What Mutation Testing Does

Mutation testing systematically breaks your code and checks whether your tests notice.

The mutation testing tool:
1. Parses your source code
2. Generates "mutants"—small, targeted changes (flip a `>` to `>=`, change `+` to `-`, return early, etc.)
3. Runs your tests against each mutant
4. Reports which mutants survived (tests still passed despite broken code)

If a mutant survives, your tests don't adequately cover that behavior. The test suite accepts code that's demonstrably wrong.

```bash
cargo mutants --in-place -f src/player/reservation.rs
```

Output might show:

```
src/player/reservation.rs:42: replace `>` with `>=` in available_balance ... SURVIVED
src/player/reservation.rs:47: replace `Ok(())` with `Err(...)` ... KILLED
```

The first mutant survived—changing `>` to `>=` didn't break any tests. That's a gap in test coverage that code coverage metrics would never reveal.

## Why This Matters for LLM-Generated Code

LLMs are pattern matchers. They've seen thousands of test files and can generate plausible-looking tests at scale. But "plausible-looking" isn't "meaningful."

Consider the failure modes:

**Tautological assertions.** The LLM generates assertions that restate the setup rather than verify behavior:
```rust
let result = calculate(5, 3);
assert_eq!(result, calculate(5, 3)); // Always passes
```

**Missing edge cases.** The LLM tests the happy path but misses boundaries:
```rust
#[test]
fn test_withdraw() {
    let mut account = Account::new(100);
    account.withdraw(50).unwrap();
    assert_eq!(account.balance(), 50);
}
// Never tests: withdraw(100), withdraw(101), withdraw(0)
```

**Implementation-coupled tests.** The LLM tests that code does what it does, not what it should do:
```rust
#[test]
fn test_hash() {
    // Tests current behavior, not correct behavior
    assert_eq!(hash("input"), 0x7a3f2b1c);
}
```

Mutation testing catches all of these. Tautological assertions don't kill mutants. Missing edge cases leave mutation gaps. Implementation-coupled tests kill mutants but the wrong ones—they're brittle to refactoring while missing actual bugs.

## The Workflow

Integrate mutation testing into the TDD cycle:

1. **Describe what you want**
2. **LLM writes tests** (not implementation)
3. **You review**: "Do these tests capture my requirements?"
4. **LLM implements** to make tests pass
5. **Run mutation testing** on the implementation
6. **Analyze survivors**: Which behaviors aren't actually tested?
7. **Iterate**: Add tests that kill survivors, or accept the gap

Step 6 is the key addition. Mutation testing provides objective feedback: "Your tests claim to verify X, but they'd accept this broken version of X."

This is a deterministic checkpoint in a non-deterministic workflow. The LLM might generate hollow tests. You might miss them in review. But the mutants don't lie.

## Practical Application

Mutation testing works best when business logic is isolated from infrastructure. That's a core design principle of [Angzarr](/): aggregates, sagas, and projectors contain pure business logic with no database calls, no network I/O, no framework dependencies. The coordinator handles infrastructure; your code handles decisions.

This isolation makes code easier to test—and easier to test *meaningfully*. When a function takes state and returns events, every branch is reachable without mocking. Mutation testing thrives in this environment.

Industry research provides concrete benchmarks for mutation kill rates:

| Context | Target Kill Rate | Source |
|---------|------------------|--------|
| Google production (at scale) | 87%+ | [State of Mutation Testing at Google][google-mutation] |
| Mature/production systems | 90% | [Pitest best practices][pitest-guide] |
| Initial adoption baseline | 70-80% | [Pitest best practices][pitest-guide] |
| Critical systems (payments, security) | 80-90% | [testRigor guide][testrigor] |
| Less critical areas | 60-70% acceptable | [testRigor guide][testrigor] |

A common shock: teams with 80-90% *code coverage* often discover mutation scores of [only 30%][diffblue] when first adopting mutation testing. That's the gap between "tests executed this code" and "tests verified this code."

In this codebase, mutation testing revealed qualitative patterns consistent with the research:

**Pure utility functions should target 80-90%+ kill rates.** Functions that transform data without side effects are fully testable. If mutants survive, the tests are incomplete.

```rust
// merge.rs - pure logic
pub fn diff_state_fields(before: &Any, after: &Any) -> HashSet<String> {
    // Every branch here should have a mutant-killing test
}
```

**Framework glue tolerates lower rates.** gRPC handlers that delegate to tested core logic don't need exhaustive mutation coverage. Integration tests cover the composition. Mutation testing [primarily targets unit tests][diffblue]; integration-heavy code may have hard-to-detect mutants.

**Surviving mutants in logging are acceptable.** If removing a `debug!()` call doesn't break tests, that's expected—logging is a side effect that doesn't affect correctness.

[google-mutation]: https://research.google/pubs/state-of-mutation-testing-at-google/
[pitest-guide]: https://bell-sw.com/blog/a-comprehensive-guide-to-mutation-testing-in-java/
[testrigor]: https://testrigor.com/blog/understanding-mutation-testing-a-comprehensive-guide/
[diffblue]: https://www.diffblue.com/resources/what-is-mutation-testing-java/

## The Meta-Workflow

Here's what this looks like end-to-end (illustrative, not a real transcript):

```
You: "Add fund reservation to prevent double-booking"

LLM writes tests:
  - test_reserve_reduces_available_balance
  - test_reserve_fails_on_insufficient_funds
  - test_release_restores_available_balance

You review: "Looks like it covers the requirements"

LLM implements Player::reserve() and Player::release()

Tests pass. Coverage: 100%

You run: cargo mutants -f src/player/reservation.rs

Results:
  - 12 mutants killed
  - 2 mutants survived:
    - "replace > with >=" on line 42
    - "remove bounds check" on line 47

Analysis: The boundary condition (exactly equal to available) isn't tested.

You: "Add a test for reserving exactly the available balance"

LLM adds: test_reserve_exact_available_balance_succeeds

Mutants re-run: 14/14 killed

Done.
```

The mutation testing step caught a gap that code coverage missed. The test suite now constrains the actual behavior, not just the happy path.

## Why Determinism Matters Here

The original post argued: use non-deterministic tools to build deterministic artifacts. Tests are deterministic—they pass or fail reproducibly.

But tests can be deterministic while being worthless. A test that always passes is perfectly reproducible. It just doesn't prove anything.

Mutation testing adds a second layer of deterministic verification: not just "do tests pass?" but "do tests actually constrain behavior?" The mutation tool doesn't guess. It systematically breaks things and observes results. Either the tests catch the breakage or they don't.

This is the deterministic arbiter you need when working with LLM-generated tests. The LLM can generate plausible tests at scale. Mutation testing determines whether those tests mean anything.

## The Investment

Mutation testing is slow. Running it on a full codebase can take hours. For incremental work:

```bash
# Only test the file you just changed
cargo mutants --in-place --timeout 120 -f src/player/reservation.rs

# Use feature flags if your tests need them
cargo mutants --in-place -f src/player/reservation.rs -- --features "sqlite test-utils"
```

The timeout matters—some mutants create infinite loops. 120 seconds catches most real tests while killing pathological cases.

Run mutation testing:
- After writing new tests (before considering them "done")
- When LLM claims high test coverage
- Before merging significant new functionality
- When you're suspicious that tests are hollow

Don't run it on every commit. That's overkill. Run it when you need confidence that tests are meaningful.

## The Punchline

The previous post established: LLMs draft, humans verify through tests.

This post adds: tests themselves need verification. Mutation testing provides that verification deterministically.

The workflow becomes:

1. LLM generates tests (constrain before implementing)
2. You review tests (verify they capture intent)
3. LLM implements (make tests pass)
4. **Mutation testing validates tests** (prove they constrain behavior)
5. Iterate until mutants are killed

The LLM accelerates drafting. Tests verify the draft. Mutation testing verifies the tests. Each layer is more deterministic than the last, building reliable systems from unreliable components.

---

*Yes, the tests for this post were also mutation-tested. The surviving mutants were in the prose.*
