# Code Review - Recent Changes

Review all changes since the last commit.

## Step 1: Examine Changes

Run `git diff HEAD~1` to see what changed (or `git diff --staged` for uncommitted changes).

## Step 2: Apply Review Perspectives

- **Architecture** - Structure, boundaries, testability, technical debt
- **Performance** - Algorithms, data structures, complexity, efficiency
- **Standards** - Readability, naming, documentation, canonical patterns
- **Concurrency** - Thread safety, race conditions, synchronization (if applicable)
- **Domain** - client logic correctness, domain model accuracy (if applicable)

## Step 3: Report Findings

### Summary
What do these changes accomplish?

### Critical Issues
Must fix before merging.

### Medium Issues
Should address but doesn't block.

### Minor Issues
Style and small improvements.

### Positive Observations
Good patterns worth noting.