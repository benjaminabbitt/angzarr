# Documentation commands

TOP := `git rev-parse --show-toplevel`

# Render documentation from templates (updates LOC counts, etc.)
render:
    @uv run "{{TOP}}/scripts/render_docs.py"

# Check if documentation is up to date (for CI)
check:
    @uv run "{{TOP}}/scripts/render_docs.py" --check

# Show what documentation would be updated
dry-run:
    @uv run "{{TOP}}/scripts/render_docs.py" --dry-run

# Show example LOC stats
loc:
    @uv run "{{TOP}}/scripts/count_example_loc.py" --format markdown
