# Release management commands
# Uses versionator CLI for semantic versioning with VERSION file

TOP := `git rev-parse --show-toplevel`

# Bump patch version (0.1.0 -> 0.1.1)
patch:
    cd "{{TOP}}" && versionator patch increment
    @uv run "{{TOP}}/scripts/sync_version.py"

# Bump minor version (0.1.0 -> 0.2.0)
minor:
    cd "{{TOP}}" && versionator minor increment
    @uv run "{{TOP}}/scripts/sync_version.py"

# Bump major version (0.1.0 -> 1.0.0)
major:
    cd "{{TOP}}" && versionator major increment
    @uv run "{{TOP}}/scripts/sync_version.py"

# Create git tag from VERSION
tag:
    cd "{{TOP}}" && versionator commit
    cd "{{TOP}}" && git push origin --tags

# Full release workflow: bump version, commit, tag, push
full TYPE="patch":
    @just release {{TYPE}}
    cd "{{TOP}}" && git add VERSION Cargo.toml deploy/helm/angzarr/Chart.yaml
    cd "{{TOP}}" && git commit -m "chore: release v$(cat VERSION)"
    just release tag

# Show current version
version:
    @cat "{{TOP}}/VERSION"
