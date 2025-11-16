#!/bin/bash
# Continuous Integration Testing Script

set -e

echo "Running Angzarr CI Tests"
echo "========================"

# Format check
echo "Checking code formatting..."
just fmt-check

# Linting
echo "Running linter..."
just lint

# Unit tests
echo "Running unit tests..."
just test

# Gherkin tests (when implemented)
# echo "Running Gherkin tests..."
# just test-gherkin

# Build all phases
echo "Building Phase 1..."
just build-phase-1

echo "Building Phase 2..."
just build-phase-2

echo "Building Phase 3..."
just build-phase-3

# Full workspace build
echo "Building full workspace..."
just build

echo "All CI tests passed!"
