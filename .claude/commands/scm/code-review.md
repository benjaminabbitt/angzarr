# Code Review

Perform a comprehensive code review applying multiple perspectives.

## Perspectives

Apply each systematically:
- **Architecture** - Structure, boundaries, testability, technical debt
- **Performance** - Algorithms, data structures, complexity, efficiency
- **Standards** - Readability, naming, documentation, canonical patterns
- **Concurrency** - Thread safety, race conditions, synchronization (if applicable)
- **Domain** - client logic correctness, domain model accuracy (if applicable)

## Process

1. Understand what the code does
2. Apply each perspective
3. Categorize findings by severity
4. Provide file:line references for all findings
5. Note positive patterns worth maintaining

## Output Format

### Summary
Brief description of what the code does.

### Critical Issues
Must fix before merging.

### Medium Issues
Should address but doesn't block.

### Minor Issues
Style and small improvements.

### Positive Observations
Good patterns worth noting.