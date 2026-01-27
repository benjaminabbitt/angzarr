# Code quality commands

TOP := `git rev-parse --show-toplevel`

# Check code
check:
    cargo check

# Format code
fmt:
    cargo fmt

# Lint code
lint:
    cargo clippy -- -D warnings

# Lint all Helm charts (main + all examples)
helm-lint:
    @echo "Linting main Helm chart..."
    helm lint "{{TOP}}/deploy/helm/angzarr"
    @echo "Linting example Helm charts..."
    just examples helm-lint

# Clean build artifacts
clean:
    cargo clean
