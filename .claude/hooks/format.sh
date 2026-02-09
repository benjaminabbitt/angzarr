#!/bin/bash
# Format code after Claude completes a task.
# Runs on TaskCompleted event.

set -euo pipefail

cd "$CLAUDE_PROJECT_DIR"

# Format Rust code
if command -v cargo &> /dev/null && [ -f Cargo.toml ]; then
    cargo fmt --all 2>/dev/null || true
fi

# Format Python code
if command -v ruff &> /dev/null; then
    ruff format . 2>/dev/null || true
fi

exit 0
