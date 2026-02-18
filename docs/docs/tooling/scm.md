---
sidebar_position: 5
---

# SCM (Smart Context Manager)

SCM is a context management tool that helps LLMs maintain relevant context across conversations. It's used throughout angzarr to provide AI assistants with project-specific knowledge.

---

## What SCM Does

- **Context fragments** — Reusable pieces of context (patterns, conventions, architecture)
- **Profiles** — Named collections of fragments for different tasks
- **Dynamic assembly** — Combine fragments based on tags or explicit selection
- **CLAUDE.md integration** — Automatically injects context into project instructions

---

## Directory Structure

```
.scm/
├── context.md           # Main assembled context (auto-generated)
├── fragments/           # Reusable context pieces
│   ├── rust-dev.md
│   ├── git-practices.md
│   ├── testing.md
│   └── ...
└── profiles/            # Named fragment collections
    ├── default.yaml
    └── review.yaml
```

---

## CLAUDE.md Integration

The project's `CLAUDE.md` includes an SCM marker:

```markdown
<!-- SCM:BEGIN -->
@.scm/context.md
<!-- SCM:END -->
```

SCM replaces this section with assembled context. The `@` syntax includes the referenced file's contents.

---

## Fragments

Fragments are markdown files containing focused context:

```markdown
# fragments/rust-dev.md

## Tooling
- Version: Cargo.toml
- Tests: `#[cfg(test)]`, cucumber-rs (Gherkin)
- Quality: clippy, rustfmt, cargo-audit

## Error Handling
- Use `Result<T, E>` + `?` operator
- NO `unwrap()`/`expect()` in library code
```

### Fragment Tags

Fragments can be tagged for selective inclusion:

```markdown
---
tags: [rust, development, testing]
---

# Rust Development
...
```

---

## Profiles

Profiles define which fragments to include:

```yaml
# profiles/default.yaml
fragments:
  - rust-dev
  - git-practices
  - testing
  - architecture

# profiles/review.yaml
fragments:
  - code-review
  - testing
  - security
```

---

## Usage

### Assemble Context

```bash
# Assemble default profile
scm assemble

# Assemble specific profile
scm assemble --profile review

# Assemble by tags
scm assemble --tags rust,testing
```

### List Fragments

```bash
scm list-fragments
scm list-fragments --tags security
```

### List Profiles

```bash
scm list-profiles
```

---

## MCP Integration

SCM provides an MCP server for AI tool access:

```json
{
  "mcpServers": {
    "scm": {
      "command": "scm",
      "args": ["mcp"]
    }
  }
}
```

This exposes tools like:
- `list_fragments` — Browse available fragments
- `get_fragment` — Read a specific fragment
- `assemble_context` — Build context from profile/tags

---

## Best Practices

1. **Keep fragments focused** — One topic per fragment
2. **Use tags consistently** — Enable flexible assembly
3. **Update regularly** — Context should reflect current practices
4. **Profile for tasks** — Different tasks need different context

---

## Next Steps

- **[Claude Code](/tooling/claude)** — AI assistant integration
- **[Getting Started](/getting-started)** — Project setup
