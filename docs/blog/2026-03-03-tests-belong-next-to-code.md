---
slug: tests-belong-next-to-code
title: "Tests Belong Next to the Code They Test"
authors: [angzarr]
tags: [testing, patterns, rust, java, architecture]
keywords: [testing, test organization, rust, java, maven, cargo, colocation, documentation]
---

Why I've always preferred tests adjacent to production code, and how modern toolchains are finally catching up.

<!-- truncate -->

## The Divisive Opinion

For most of my career, I've held a divisive opinion: test code should live next to the code it tests. Not in a parallel directory tree. Not in a separate package. *Right there*, where you can see it.

This puts me at odds with decades of enterprise convention. Java's Maven established the `src/main` and `src/test` split. .NET followed. The pattern became orthodoxy: production code here, test code over there, and never the twain shall meet.

I think this was a mistake born from [tooling limitations](https://maven.apache.org/guides/introduction/introduction-to-the-standard-directory-layout.html), not from any inherent benefit to developers.

## The Case for Colocation

When tests live next to code, several good things happen:

**Tests are visible.** Open a file, see its tests. No hunting through a parallel directory structure. No wondering "does this even have tests?" The presence (or absence) of tests is immediately apparent.

**Tests serve as documentation.** Well-structured tests show how code is meant to be used. When tests are one scroll away, they're more likely to be read. When they're three directories over, they're discovered only when something breaks.

**Refactoring is safer.** Move a file, its tests move with it. Rename a module, the tests follow. The parallel directory structure creates a fragile coupling: rename `src/main/java/com/example/UserService.java` and you'd better remember to rename `src/test/java/com/example/UserServiceTest.java` too.

**Code review is easier.** When reviewing a change, the test changes are contextually adjacent. You see the implementation and its verification together. With split directories, reviewers must mentally correlate changes across distant locations.

**Tests remind you they exist.** Out of sight, out of mind. Tests in a parallel tree are easy to forget, easy to neglect, easy to let rot. Tests next to code are a constant presence, a reminder that this code has expectations that must be maintained.

## The Java Problem

Java's `src/main/java` and `src/test/java` split became the template for an entire generation of build tools. [Maven codified it](https://maven.apache.org/guides/introduction/introduction-to-the-standard-directory-layout.html). [Gradle inherited it](https://docs.gradle.org/current/userguide/java_plugin.html). IDE project wizards generate it by default.

```
my-project/
├── src/
│   ├── main/
│   │   └── java/
│   │       └── com/
│   │           └── example/
│   │               └── UserService.java
│   └── test/
│       └── java/
│           └── com/
│               └── example/
│                   └── UserServiceTest.java
└── pom.xml
```

To find the tests for `UserService`, you navigate:
1. Up from `src/main/java/com/example/`
2. Over to `src/test/java/`
3. Back down through `com/example/`
4. Find `UserServiceTest.java`

That's not "next to the code." That's an archaeological expedition.

Worse, the parallel structure creates maintenance burden. The package hierarchy must be duplicated exactly. Add a new package in main? Manually create the same package in test. Refactor a package name? Do it twice. This isn't separation of concerns; it's separation of location, which only adds friction.

**Why did Java do this?**

The JVM's class loading model made this nearly inevitable. Here's the chain of constraints:

1. **The JVM [loads classes from the classpath](https://docs.oracle.com/javase/8/docs/technotes/tools/findingclasses.html).** At runtime, there's no concept of "source directories," just a flat namespace of classes resolved from JAR files and directories.

2. **JAR files are the deployment unit.** You ship a `.jar` containing your compiled `.class` files. That's what goes to production.

3. **The JVM has no conditional compilation.** Unlike C's [`#ifdef`](https://gcc.gnu.org/onlinedocs/cpp/Ifdef.html) or Rust's `#[cfg]`, Java has no mechanism to say "compile this class, but exclude it from the final artifact." Every `.class` file in your source tree gets compiled. Every compiled class *could* end up in the JAR.

4. **Test dependencies are heavy.** [JUnit](https://junit.org/), [Mockito](https://site.mockito.org/), assertion libraries: these add megabytes to your classpath. You don't want them in production.

The only solution available was **physical separation**. Put test sources in a different directory tree. Compile them to a different output directory. Configure the packager to ignore that output. The directory structure *is* the filter.

Maven's [Surefire plugin](https://maven.apache.org/surefire/maven-surefire-plugin/) runs tests from `target/test-classes`. The [JAR plugin](https://maven.apache.org/plugins/maven-jar-plugin/) packages from `target/classes`. They never overlap because the *source directories* never overlapped. Physical separation at the source level cascades to physical separation at every subsequent stage.

It solved the technical problem. But it created an organizational one that we've been living with for 25 years.

## The .NET Pattern

.NET followed Java's lead, though with its own twist: **separate assemblies**.

In the .NET world, the deployment unit is the assembly (`.dll` or `.exe`). Visual Studio's project model [encourages a one-to-one correspondence](https://www.red-gate.com/simple-talk/development/dotnet-development/partitioning-your-code-base-through-net-assemblies-and-visual-studio-projects/) between projects and assemblies. Each project compiles to one assembly.

The conventional structure:
```
MySolution/
├── MyApp/
│   └── MyApp.csproj          → MyApp.dll
├── MyApp.Tests/
│   └── MyApp.Tests.csproj    → MyApp.Tests.dll
└── MySolution.sln
```

Why separate projects instead of separate directories within one project?

1. **Assembly references are explicit.** `MyApp.Tests.csproj` references `MyApp.csproj`. The test assembly depends on the production assembly. This is cleaner than trying to exclude certain source files from compilation.

2. **NuGet packages are per-project.** Test frameworks ([xUnit](https://xunit.net/), [NUnit](https://nunit.org/), [MSTest](https://learn.microsoft.com/en-us/dotnet/core/testing/unit-testing-with-mstest)) are NuGet dependencies. You want them in the test project, not the production project. Separate projects mean separate dependency graphs.

3. **Deployment is per-assembly.** When you deploy, you copy assemblies. If tests were in the same assembly, you'd need post-build filtering, something MSBuild never standardized.

The result: even more separation than Java. Not just different directories, but entirely different projects. The test code isn't merely in a parallel tree; it's in a parallel *solution structure*.

[Microsoft's guidance](https://learn.microsoft.com/en-us/dotnet/core/tools/dotnet-pack) for `dotnet pack` assumes this separation: "OctoPack should only be installed on projects that you are going to deploy... Do not install OctoPack on unit tests, class libraries, or other supporting projects."

The tooling *expects* you to keep tests far away from production code.

## The Rust Innovation: True Conditional Compilation

Rust solved the problem Java and .NET couldn't: **compile-time code elimination**. The language's [test organization](https://doc.rust-lang.org/book/ch11-03-test-organization.html) is built around this capability.

```rust
// user_service.rs

pub struct UserService {
    // ...
}

impl UserService {
    pub fn create_user(&self, name: &str) -> Result<User, Error> {
        // implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_succeeds() {
        let service = UserService::new();
        let user = service.create_user("Alice").unwrap();
        assert_eq!(user.name, "Alice");
    }

    #[test]
    fn test_create_user_rejects_empty_name() {
        let service = UserService::new();
        let result = service.create_user("");
        assert!(result.is_err());
    }
}
```

The [`#[cfg(test)]`](https://doc.rust-lang.org/reference/conditional-compilation.html) attribute tells the compiler: "this module exists only when compiling for tests." In release builds, this code doesn't exist. Zero overhead. No separate directories needed.

**How it actually works:**

Rust's conditional compilation operates at the AST (Abstract Syntax Tree) level, before code generation. When the compiler encounters `#[cfg(test)]`:

1. It evaluates the predicate (`test`) against the current compilation configuration
2. If false (release build), [the entire annotated item is removed from the source](https://doc.rust-lang.org/reference/conditional-compilation.html). Not compiled, not linked, not present in the binary.
3. If true (test build), the attribute is stripped and compilation proceeds normally

This is fundamentally different from Java's approach. Java compiles everything, then uses directory conventions to exclude artifacts. Rust *doesn't compile* the excluded code. The test module isn't "excluded from the JAR"; it never becomes machine code in the first place.

The implications:

- **No deployment risk.** You can't accidentally ship test code because it doesn't exist in the release binary. There's nothing to exclude.
- **No dependency contamination.** Test-only dependencies (marked [`[dev-dependencies]`](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies) in `Cargo.toml`) are only linked when building tests. Release builds don't see them.
- **Source-level colocation.** Since the compiler handles exclusion, there's no need for physical separation. Tests can live in the same file, and the tooling handles the rest.

**This isn't preprocessor hackery.** Unlike C's [`#ifdef`](https://gcc.gnu.org/onlinedocs/cpp/Ifdef.html), which operates on text before parsing, Rust's `#[cfg]` is a proper language feature. The compiler understands the structure. IDEs can gray out inactive code. Refactoring tools work correctly across conditional boundaries.

**The benefits are immediate:**

- Open `user_service.rs`, scroll down, see the tests
- Refactor `UserService`? The tests are right there to update
- Code review shows implementation and tests in one diff
- IDE navigation is trivial: it's the same file

**But doesn't this make files huge?**

Sometimes. Rust developers have split opinions. Some keep tests inline. Others use a `tests` submodule in a [separate file](https://doc.rust-lang.org/rust-by-example/cargo/test.html). The language supports both:

```rust
// user_service.rs
pub struct UserService { /* ... */ }

#[cfg(test)]
mod tests;  // Load tests from user_service/tests.rs
```

Either way, the tests are *adjacent*: same directory, obvious relationship, no parallel tree.

## Go's Middle Ground: Convention Over Configuration

Go took a different approach than both Java and Rust: **file naming conventions**.

```
mypackage/
├── user_service.go           # Production code
└── user_service_test.go      # Test code
```

Files ending in `_test.go` are [only compiled by `go test`](https://pkg.go.dev/testing). The regular `go build` command ignores them entirely. No annotations, no configuration, no separate directories. Just a naming convention that the toolchain respects.

**How it works:**

The Go toolchain examines file names before compilation. When you run `go build`:
- `user_service.go` → compiled
- `user_service_test.go` → skipped

When you run `go test`:
- `user_service.go` → compiled
- `user_service_test.go` → compiled

The convention extends to test packages. A file in package `mypackage` can have tests in the same package (white-box testing) or in `mypackage_test` (black-box testing). Both live in the same directory:

```go
// user_service_test.go
package mypackage  // Same package: access to unexported identifiers

// user_service_external_test.go
package mypackage_test  // Different package: only exported API visible
```

**Why this works for Go:**

1. **Static compilation.** Go compiles to native binaries. There's no runtime class loading to worry about. If a file isn't compiled, its code doesn't exist.

2. **Flat package structure.** Go packages are single directories. No nested hierarchies to mirror. The test file sits next to its source file naturally.

3. **Convention enforcement.** The toolchain *is* the convention. You don't configure anything; you just name files correctly. Developers can't accidentally break it.

**The tradeoff:**

Go's approach is less flexible than Rust's. You can't conditionally compile *part* of a file; the entire `_test.go` file is in or out. For fine-grained conditional compilation, you'd need [build constraints](https://pkg.go.dev/go/build#hdr-Build_Constraints) (a separate feature). But for test organization, the simplicity wins.

Tests are colocated. No parallel directories. No configuration. Just `foo.go` and `foo_test.go`, side by side.

## The Principle: Smart Tooling Over Physical Separation

The insight isn't "Rust good, Java bad." It's that **physical separation is a workaround for tooling limitations**.

Look at the pattern:

| Language | Tooling Limitation | Workaround |
|----------|-------------------|------------|
| Java | No conditional compilation; classpath-based class loading | Parallel directory trees (`src/main` vs `src/test`) |
| .NET | Assembly-based deployment; per-project dependencies | Separate projects (`MyApp` vs `MyApp.Tests`) |
| Rust | *None*—`#[cfg]` eliminates code at compile time | Tests in same file |
| Go | *None*—`_test.go` convention excludes files from build | Tests in same directory |

Java and .NET adopted physical separation because their toolchains couldn't selectively exclude code. The directory structure *became* the exclusion mechanism. It wasn't a design choice for developer ergonomics; it was the only option available.

Rust and Go proved that smarter tooling removes the need for separation. Rust's [`#[cfg(test)]`](https://doc.rust-lang.org/reference/conditional-compilation.html) eliminates code before it's compiled. Go's [`_test.go` suffix](https://pkg.go.dev/testing) makes file exclusion a first-class toolchain feature. Neither requires parallel directories because the compiler handles what the filesystem used to handle.

**The lesson:** When evaluating test organization patterns, ask whether the pattern exists for developer benefit or tooling necessity. If it's the latter, check whether your toolchain has evolved past the limitation. You might be inheriting ceremony from a constraint that no longer applies.

## What We Do in This Project

We've landed on a middle ground for Rust: tests in `.test.rs` files adjacent to source files.

```
src/
├── correlation.rs           # Production code
├── correlation.test.rs      # Tests for correlation.rs
├── validation.rs
├── validation.test.rs
└── mod.rs
```

The parent module conditionally includes the test file:

```rust
// mod.rs
pub mod correlation;

#[cfg(test)]
#[path = "correlation.test.rs"]
mod correlation_tests;
```

This gives us:
- Tests adjacent to code (same directory)
- Production files stay focused on implementation
- Test files can be skipped when reading for understanding
- Still conditionally compiled via `#[cfg(test)]`
- No parallel directory tree
- Clean mutation testing workflow

**Mutation testing benefits**

Separate test files pair well with mutation testing tools like [cargo-mutants](https://mutants.rs/). Should tooling fail and mutations survive test completion (accidentally getting committed), they're easy to detect and restore. The mutation is in `correlation.rs`; the test file `correlation.test.rs` is untouched. Revert the production file, keep the tests.

With inline tests, a surviving mutation contaminates the same file as your test code. Reverting means losing both the mutated production code *and* any test improvements made in the same session. Separate files mean clean boundaries: production code changes stay isolated from test code changes.

It's not quite "tests in the same file," but it's close enough. Open the directory, see both files, understand the relationship. The key property, colocation, is preserved.

**Multiple test files per source file**

The `#[path]` pattern also allows multiple test files for a single source file:

```rust
// mod.rs
pub mod correlation;

#[cfg(test)]
#[path = "correlation.test.rs"]
mod correlation_unit_tests;

#[cfg(test)]
#[path = "correlation.contract.test.rs"]
mod correlation_contract_tests;
```

We don't currently use this, and needing it is generally a code smell—if your source file needs multiple test files, the source file is probably doing too much. But the option exists for cases where it's genuinely valuable: separating fast unit tests from slower contract tests, or organizing tests by behavior category when a module legitimately has broad responsibilities.

## When Separation Makes Sense

I'm not absolutist about this. Some testing scenarios benefit from separation—and understanding where tests belong is part of understanding the [testing pyramid](https://martinfowler.com/bliki/TestPyramid.html):

**Integration tests** that exercise multiple modules often belong in a dedicated `tests/` directory. They're not testing one file; they're testing interactions.

**End-to-end tests** that spin up the whole system are genuinely different from unit tests. Different lifecycle, different dependencies, different execution context.

**Test fixtures and utilities** shared across many tests might warrant their own module. Though even then, I'd put them in `src/test_utils/` rather than a parallel tree.

The principle isn't "never separate." It's "don't separate without reason." The default should be colocation. Separation should be a deliberate choice, not a convention inherited from tools that couldn't do better.

## The Documentation Argument

Well-written tests are documentation. They show:
- How to construct objects
- What inputs are valid
- What outputs to expect
- What error conditions exist
- What edge cases matter

When tests are next to code, this documentation is discoverable. Developers reading the implementation naturally encounter the tests. They see examples. They understand intent.

When tests are in a parallel tree, this documentation might as well not exist. Developers find it only when tests fail. The learning opportunity is lost.

Consider: how often do you proactively navigate to `src/test/java/...` to understand how a class works? Now compare: how often would you scroll down in a file you're already reading?

Colocation turns tests into documentation that developers actually encounter.

## The Tooling Trend

The industry is slowly moving toward colocation. JavaScript's [Jest popularized `*.test.js` files next to source](https://jestjs.io/docs/configuration). Go [requires `*_test.go` in the same package](https://pkg.go.dev/testing). Rust puts tests in the same file by default.

Even Java is shifting. [JUnit 5](https://junit.org/junit5/docs/current/user-guide/) supports test classes in the same package as production code (with proper module configuration). Gradle can be [configured to compile tests](https://docs.gradle.org/current/userguide/java_testing.html) from custom source sets. It's not the default, but it's possible.

The old separation was a tooling constraint. As tooling improves, the constraint lifts. The question becomes: what organization actually serves developers best?

My answer, after two decades: tests belong next to the code they test. Always have. The tools are finally catching up.

## The AI Context Window Changes Everything

Here's where I admit my thinking has evolved.

For years, I advocated for tests *in the same file*. Rust's `#[cfg(test)] mod tests` at the bottom of each file was, I thought, the ideal. Maximum colocation. Maximum visibility. One file, one scroll, everything you need.

Then I started working extensively with AI coding assistants.

AI tools operate within context windows—a fixed budget of tokens they can "see" at once. Every line of code loaded into context is a line that competes for attention. When an AI reads a 500-line file where 300 lines are tests, it's spending 60% of its context budget on code that's irrelevant to most tasks.

**The problem isn't that tests exist. It's that they're in the way.**

When I ask an AI to "understand how UserService handles authentication," it doesn't need to see 47 test cases. It needs the implementation. But if tests are inline, the AI loads them anyway. Context fills up. Important code gets truncated or summarized. The AI's understanding degrades.

Worse, when searching for business logic across a codebase, inline tests create noise. "Find where we validate email addresses" returns hits in test assertions, test fixtures, test helpers—all irrelevant to understanding the production validation logic.

### The New Position: Separate Files, Same Directory

I've updated my stance. Tests should be:
- **In separate files** — not inline with production code
- **In the same directory** — not in a parallel tree
- **Clearly named** — `.test.rs`, `_test.go`, `.test.ts`

```
src/
├── user_service.rs           # Production code only
├── user_service.test.rs      # Tests only
├── validation.rs
├── validation.test.rs
└── mod.rs
```

This preserves colocation—tests are *adjacent*, one directory listing shows them together—while enabling selective loading. An AI (or human) exploring business logic can skip `.test` files entirely. When it's time to maintain tests, they're right there.

### Instructing AI to Skip Test Files

The key insight: **you can tell AI tools what to ignore**.

In our project instructions, we now include guidance like:

> When searching for business logic or understanding how features work, skip `.test.rs` files. These contain tests, not implementation. Only read test files when specifically working on test maintenance or verification.

This simple instruction dramatically improves AI efficiency. A search for "correlation ID propagation" no longer returns 50 test files asserting correlation IDs are propagated. It returns the 3 files where propagation actually happens.

The tests still exist. They're still adjacent. They're still discoverable. But they're not polluting every context window and every search result.

### The Tradeoff I'm Making

Let me be honest about what's lost:

**Inline tests were more visible.** You couldn't miss them—they were right there when you opened the file. Separate files require knowing to look for them.

**Inline tests encouraged reading.** Scroll down, see tests, understand usage. With separate files, there's an extra step.

**Inline tests were atomically versioned.** Change the function, change the test, one commit. Separate files technically allow them to drift (though tooling and discipline prevent this).

These are real costs. I'm trading them for:

- **Cleaner production files** — implementation without test noise
- **Efficient AI assistance** — context windows focused on relevant code
- **Faster codebase search** — grep for logic, not test assertions
- **Flexible reading** — choose when to engage with tests

For me, in 2026, with AI assistants as daily collaborators, the tradeoff favors separate files.

### Testcontainers Blur the Lines

There's another shift happening that affects test organization: testcontainers.

Traditionally, we drew a hard line between unit tests and integration tests:

- **Unit tests**: Fast, no external dependencies, run anywhere, colocate with code
- **Integration tests**: Slow, need databases/queues/services, run in CI, separate directory

This separation made sense when "integration test" meant "spin up a full environment." You wouldn't colocate tests that require PostgreSQL next to your repository implementation—they'd fail on every developer's machine without the right setup.

[Testcontainers](https://testcontainers.com/) changed this. (In Rust, we use [testcontainers-rs](https://docs.rs/testcontainers/).)

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

**What does this mean for test organization?**

Tests that verify trait implementations—`EventStore`, `SnapshotStore`, `MessageBus`—are no longer "integration tests" in the traditional sense. They're interface contract tests. They verify that a specific implementation correctly fulfills its contract.

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

**The new distinction isn't unit vs integration—[it's scope](https://martinfowler.com/articles/practical-test-pyramid.html).**

| Test Type | What It Tests | Where It Lives |
|-----------|--------------|----------------|
| Unit | Pure logic, no dependencies | Adjacent `.test` file |
| Contract | Single implementation against its interface | Adjacent `.test` file (with testcontainers) |
| Integration | Multiple components interacting | `tests/` directory |
| End-to-end | Full system behavior | Separate test project |

Contract tests with testcontainers are closer to unit tests than integration tests. They test one thing. They're fast enough to run frequently. They should be colocated.

**The CI consideration**

Yes, testcontainer tests are slower than pure unit tests. On my machine, a PostgreSQL container adds ~2 seconds of startup. That's too slow for "run on every save" but fine for "run before commit."

We handle this with test categories:

```rust
#[test]
fn test_pure_logic() { /* runs always */ }

#[test]
#[cfg_attr(not(feature = "testcontainers"), ignore)]
fn test_postgres_storage() { /* runs with --features testcontainers */ }
```

Local development runs the fast tests continuously. [Pre-commit hooks](https://pre-commit.com/) and CI run everything. The slower tests are still colocated—they're just conditionally executed.

**Mocks are for boundaries, not implementations**

This shift has changed how I think about mocking. Previously, I'd mock the database to test repository logic. Now I test the repository against a real database (via testcontainers) and reserve mocks for:

- External services I don't control (third-party APIs)
- Failure injection (simulate network errors)
- True unit tests of pure logic

If I *can* test against the real thing cheaply, I should. Testcontainers made "the real thing" cheap.

### This Isn't a Reversal

I still believe tests belong *next to* code—same directory, obvious relationship, no parallel tree structure. I still reject the `src/main`/`src/test` split.

What's changed is the file boundary. Tests adjacent in the directory structure, but not inline in the same file. Colocation without conflation.

The principle remains: **tests should be discoverable and contextually related to the code they test**. The implementation adapts to new constraints—specifically, the constraint that every token in an AI's context window has a cost.

### This Won't Be the Answer Forever

I'm under no illusion that separate-but-adjacent test files are the final word. This is the best solution *now*, given:
- Current AI context window sizes
- Current toolchain capabilities
- Current mutation testing workflows
- Current code review practices

Every position in this article emerged from tooling constraints of its era. Java's parallel directories made sense when the JVM couldn't exclude code. Rust's inline tests made sense when file size didn't compete with AI context budgets. My current position makes sense given today's tradeoffs.

Tomorrow's tradeoffs will be different. AI context windows will grow. IDE integrations will get smarter about selective loading. New testing paradigms will emerge. When the constraints change, the optimal organization will change too.

What I'm confident won't change: **tests belong near the code they test**. The definition of "near" adapts to tooling. The principle doesn't.

## Try It

If your toolchain supports it:

1. Put a test file next to your source file
2. Configure conditional compilation or build exclusion
3. See if the proximity changes how you think about testing

You might find, as I did, that tests stop feeling like a chore in a distant directory and start feeling like a natural part of the code itself.
