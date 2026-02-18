---
sidebar_position: 6
---

# Claude Code

Angzarr uses Claude Code (Anthropic's CLI tool) for AI-assisted development. Project-specific instructions are maintained in `CLAUDE.md` files.

---

## CLAUDE.md Files

Claude Code reads instructions from `CLAUDE.md` files at multiple levels:

```
~/CLAUDE.md                    # User-level (global preferences)
~/workspace/angzarr/CLAUDE.md  # Project-level (angzarr-specific)
```

Project-level instructions override user-level for that codebase.

---

## Angzarr Instructions

The angzarr `CLAUDE.md` includes:

### Code Patterns

- **guard/validate/compute** — Handler structure for aggregates
- **Error constants** — Centralized error messages
- **IoC patterns** — Dependency injection via generics

### Testing

- **TDD mandatory** — Red/green/refactor cycle
- **Three levels** — Unit, integration, acceptance
- **Gherkin specs** — Living documentation

### Git Practices

- **No AI attribution** — Commits don't mention Claude/Anthropic
- **lefthook** — Pre-commit hooks for lint/format/test

---

## SCM Integration

`CLAUDE.md` includes assembled context from SCM:

```markdown
<!-- SCM:BEGIN -->
@.scm/context.md
<!-- SCM:END -->
```

This keeps AI context in sync with project practices. See [SCM](/tooling/scm) for details.

---

## Custom Skills

Claude Code supports custom skills (slash commands). Angzarr defines:

### `/nuke-deploy`

Tear down, rebuild from scratch, and redeploy to the Kind cluster.

### `/code-review`

Comprehensive code review following project standards.

### `/code-review-recent`

Review recent changes (uncommitted or last commit).

---

## Hooks

Claude Code hooks execute shell commands in response to events:

```json
{
  "hooks": {
    "pre-commit": ["just check", "just fmt --check"],
    "post-tool-use": {
      "Write": ["just fmt {{file}}"]
    }
  }
}
```

Hooks ensure code quality without manual intervention.

---

## MCP Servers

Claude Code can connect to MCP servers for extended capabilities:

```json
{
  "mcpServers": {
    "scm": {
      "command": "scm",
      "args": ["mcp"]
    },
    "mcp-tasks": {
      "command": "mcp-tasks"
    }
  }
}
```

### Available MCPs

- **scm** — Context fragment management
- **mcp-tasks** — Task tracking in markdown files

---

## Best Practices

### Keep Instructions Current

Update `CLAUDE.md` when project practices change. Stale instructions cause AI confusion.

### Be Explicit

Claude follows instructions literally. Vague guidance produces inconsistent results.

### Use Fragments

Large `CLAUDE.md` files become unwieldy. Use SCM fragments for modular context.

### Test with AI

After updating instructions, test common tasks to verify AI behavior matches expectations.

---

## Next Steps

- **[SCM](/tooling/scm)** — Context fragment management
- **[Getting Started](/getting-started)** — Project setup with Claude Code
