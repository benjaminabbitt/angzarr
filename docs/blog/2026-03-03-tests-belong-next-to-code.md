---
slug: tests-belong-next-to-code
title: "Tests Belong Next to the Code They Test"
authors: [angzarr]
tags: [testing, patterns, rust, java, architecture]
keywords: [testing, test organization, rust, java, maven, cargo, colocation, documentation]
---

Tests should live next to the code they test—same directory, separate file. Not inline. Not in a parallel tree.

<!-- truncate -->

```
src/
├── user_service.rs           # Production code only
├── user_service.test.rs      # Tests only
└── mod.rs
```

AI context windows changed my thinking. When an AI reads a 500-line file where 300 lines are tests, it wastes 60% of its context budget on code irrelevant to most tasks. Separate files let AI skip tests; inline tests force everything into context.

Java's `src/main`/`src/test` split goes too far—that was a workaround for the JVM's inability to exclude code at compile time. Modern languages (Rust, Go) solved this. We get colocation without the baggage.

**The principle:** Tests belong near code. **The implementation:** Same directory, separate file, clearly named (`.test.rs`, `_test.go`).

---

## Why Separate Files?

I used to prefer Rust's `#[cfg(test)] mod tests` pattern: maximum colocation, one scroll shows everything.

Working with AI assistants changed my mind. Every token in an AI context window has a cost. Inline tests create noise: search for business logic, get hits in test assertions, fixtures, helpers. Ask an AI to understand authentication, it loads 47 test cases it doesn't need.

**The problem isn't that tests exist. It's that inline tests are in the way.**

Separate files preserve colocation (one directory listing shows both) while enabling selective loading. AI tools skip `.test` files. Humans wanting documentation head *for* the tests. Choice instead of force.

## Why Not Parallel Trees?

Java's `src/main`/`src/test` split was a workaround for tooling limitations, not a design choice for developer benefit.

### The Java Constraint

The JVM's class loading model forced physical separation:

1. **No conditional compilation.** Unlike Rust's `#[cfg]`, Java can't say "compile this class but exclude it from the JAR." Every `.class` file could end up in production.

2. **Heavy test dependencies.** JUnit, Mockito, assertion libraries add megabytes. You don't want them shipped.

3. **Classpath-based loading.** The only way to exclude code was to put it in a different directory and configure the packager to ignore it.

Maven's [Surefire plugin](https://maven.apache.org/surefire/maven-surefire-plugin/) runs tests from `target/test-classes`. The [JAR plugin](https://maven.apache.org/plugins/maven-jar-plugin/) packages from `target/classes`. They never overlap because the source directories never overlapped. Physical separation at source level cascades to physical separation everywhere.

```
my-project/
├── src/
│   ├── main/java/com/example/UserService.java
│   └── test/java/com/example/UserServiceTest.java
└── pom.xml
```

To find tests for `UserService`: up from `src/main/java/com/example/`, over to `src/test/java/`, back down through `com/example/`. That's not "next to the code." That's an archaeological expedition.

### The .NET Constraint

.NET went further: **separate assemblies**.

```
MySolution/
├── MyApp/MyApp.csproj          → MyApp.dll
├── MyApp.Tests/MyApp.Tests.csproj    → MyApp.Tests.dll
└── MySolution.sln
```

Assembly references are explicit. NuGet packages are per-project. Deployment is per-assembly. The tooling expects tests far away from production code.

Both patterns solved real technical problems. But they created organizational ones we've been living with for decades.

## Modern Solutions

Rust and Go proved smarter tooling removes the need for separation.

### Rust: Conditional Compilation

Rust's [`#[cfg(test)]`](https://doc.rust-lang.org/reference/conditional-compilation.html) eliminates code at compile time:

```rust
pub struct UserService { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user() { /* ... */ }
}
```

In release builds, the test module doesn't exist—not compiled, not linked, not present. Test dependencies ([`[dev-dependencies]`](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies)) are only linked when building tests.

No deployment risk. No dependency contamination. No separate directories needed.

### Go: Naming Convention

Go uses file naming: `_test.go` files are [only compiled by `go test`](https://pkg.go.dev/testing).

```
mypackage/
├── user_service.go           # Production code
└── user_service_test.go      # Test code
```

No annotations, no configuration. The toolchain examines file names before compilation. `go build` skips test files entirely.

Both languages achieve colocation because the compiler handles what the filesystem used to handle.

## What We Do

We use Rust's `.test.rs` pattern with the [`#[path]`](https://doc.rust-lang.org/reference/items/modules.html#the-path-attribute) attribute:

```
src/
├── correlation.rs           # Production code
├── correlation.test.rs      # Tests
└── mod.rs
```

```rust
// mod.rs
pub mod correlation;

#[cfg(test)]
#[path = "correlation.test.rs"]
mod correlation_tests;
```

This gives us:
- Tests adjacent to code (same directory)
- Production files focused on implementation
- Test files skippable when reading for understanding
- Conditional compilation via `#[cfg(test)]`
- Clean mutation testing workflow

**Mutation testing benefits:** Separate files pair well with tools like [cargo-mutants](https://mutants.rs/). If a mutation survives (accidentally gets committed), it's in `correlation.rs`; the test file is untouched. Revert the production file, keep the tests. With inline tests, reverting means losing both mutated code and test improvements.

## Test Support Files: When Production Needs Test Logic

Sometimes production code needs to call test-specific logic—mock handlers, test fixtures, specialized parsers for test data. The `#[cfg(test)]` block inside the production function works, but what if it's substantial? Inline test code pollutes the production file.

The solution: **test support files** using the same `#[path]` pattern.

```
src/orchestration/aggregate/
├── merge.rs                 # Production code (clean)
├── merge_test_support.rs    # Test helpers (separate file)
├── tests.rs                 # Unit tests
└── mod.rs
```

The production file stays minimal:

```rust
// merge.rs - only 4 lines of test boilerplate
#[cfg(test)]
#[path = "merge_test_support.rs"]
pub(crate) mod test_support;

pub(crate) fn diff_state_fields(before: &Any, after: &Any) -> HashSet<String> {
    // ...

    #[cfg(test)]
    if before.type_url == "test.StatefulState" {
        return test_support::diff_test_state_fields(&before.value, &after.value);
    }

    // ... production logic
}
```

The test support file contains the helpers:

```rust
// merge_test_support.rs
//! Test support for merge module.
//! Only compiled during tests via #[path] include.

pub(crate) fn parse_test_state_fields(s: &str) -> HashMap<String, String> { /* ... */ }
pub(crate) fn diff_test_state_fields(before: &[u8], after: &[u8]) -> HashSet<String> { /* ... */ }
```

Unit tests import from the support module:

```rust
// tests.rs
use super::merge::test_support::{diff_test_state_fields, parse_test_state_fields};

#[test]
fn test_diff_detects_single_change() {
    let changed = diff_test_state_fields(
        r#"{"field_a":100,"field_b":200}"#.as_bytes(),
        r#"{"field_a":100,"field_b":300}"#.as_bytes(),
    );
    assert!(changed.contains("field_b"));
}
```

**When to use this pattern:**
- Production code needs conditional test behavior
- Test helpers exceed ~20 lines
- You want readers to see business logic, not test fixtures

**Visibility note:** Use `pub(crate)` if sibling test modules need access; `pub(super)` if only the parent module calls the helpers.

This reduced our `merge.rs` from ~300 lines to ~205 lines—all test code now lives in adjacent files, still colocated but not inline.

**Context window impact:** The same principle from the intro applies here. When an AI assistant reads `merge.rs` to understand commutative merge logic, it gets 205 lines of business logic—not 300 lines where a third is test fixture parsing. The `_test_support.rs` file exists for when context *needs* test helpers; otherwise it's skipped. Every line of test code in a production file is a line competing for attention in a context window that could hold actual implementation details.

## When Separation Makes Sense

This isn't absolutism. Some tests benefit from separation:

**Integration tests** exercising multiple modules belong in `tests/`. They're not testing one file.

**End-to-end tests** spinning up the whole system are genuinely different. Different lifecycle, different dependencies.

**Shared fixtures** might warrant their own module—though I'd put them in `src/test_utils/`, not a parallel tree.

The principle: don't separate without reason. Colocation is the default. Separation is a deliberate choice.

## The Tradeoffs

What's lost with separate files:

- **Visibility.** Inline tests were impossible to miss. Separate files require knowing to look.
- **Encouragement.** Scroll down, see tests. With separate files, there's an extra step.
- **Atomic versioning.** Change function and test in one commit. Separate files technically allow drift.

What's gained:

- **Cleaner production files.** Implementation without test noise.
- **Efficient AI assistance.** Context windows focused on relevant code.
- **Faster codebase search.** Grep for logic, not test assertions.
- **Flexible reading.** Choose when to engage with tests.

For me, in 2026, with AI assistants as daily collaborators, the tradeoff favors separate files.

## This Won't Be the Answer Forever

Every position in this article emerged from tooling constraints of its era. Java's parallel directories made sense when the JVM couldn't exclude code. Rust's inline tests made sense when file size didn't compete with AI context budgets.

Tomorrow's tradeoffs will differ. AI context windows will grow. IDE integrations will get smarter. When constraints change, optimal organization changes too.

What won't change: **tests belong near the code they test**. The definition of "near" adapts to tooling. The principle doesn't.
