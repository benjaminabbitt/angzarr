# Angzarr Linux Kernel Rust Migration Build System

# Default recipe - show available commands
default:
    @just --list

# Build all crates
build:
    cargo build --workspace

# Build with release optimizations
build-release:
    cargo build --workspace --release

# Build with kernel profile (maximum optimization)
build-kernel:
    cargo build --workspace --profile kernel

# Build bootable kernel binary
build-kernel-bin:
    @echo "Building bootable kernel..."
    cd angzarr-kernel && cargo build --release
    @echo "Kernel binary: angzarr-kernel/target/x86_64-unknown-none/release/angzarr-kernel"

# Create bootable ISO image
build-iso: build-kernel-bin
    @echo "Creating bootable ISO..."
    ./scripts/build-iso.sh

# Run kernel in QEMU
run-kernel: build-iso
    @echo "Running kernel in QEMU..."
    qemu-system-x86_64 -cdrom angzarr.iso -serial stdio

# Run kernel in QEMU with debug
debug-kernel: build-iso
    @echo "Running kernel in QEMU with debugging..."
    qemu-system-x86_64 -cdrom angzarr.iso -serial stdio -s -S

# Run all unit tests
test:
    cargo test --workspace

# Run tests with output
test-verbose:
    cargo test --workspace -- --nocapture

# Run tests for a specific crate
test-crate crate:
    cargo test -p {{crate}}

# Run Gherkin/Cucumber tests
test-gherkin:
    cargo test -p angzarr-test-framework --test kernel_tests

# Run all tests (unit + gherkin)
test-all: test test-gherkin

# Check code without building
check:
    cargo check --workspace

# Run clippy linter
lint:
    cargo clippy --workspace -- -D warnings

# Format code
fmt:
    cargo fmt --all

# Check formatting without modifying
fmt-check:
    cargo fmt --all -- --check

# Clean build artifacts
clean:
    cargo clean

# Generate documentation
doc:
    cargo doc --workspace --no-deps --open

# Run property-based tests
test-prop:
    cargo test --workspace --features proptest

# Build C-compatible static libraries
build-ffi:
    cargo build --workspace --release
    @echo "FFI libraries built in target/release/"

# Run security audit
audit:
    cargo audit

# Check for outdated dependencies
outdated:
    cargo outdated

# Verify ABI compatibility
check-abi:
    @echo "Checking ABI compatibility..."
    cargo test -p angzarr-abi-test
    @echo "✅ ABI compatibility verified"

# Build kernel module (placeholder)
build-module:
    @echo "Building kernel module..."
    just build-kernel
    @echo "Module build complete"

# Run in Xen hypervisor (placeholder)
test-xen:
    @echo "Testing in Xen hypervisor..."
    @echo "Xen testing infrastructure coming soon"
    # TODO: Implement Xen boot testing

# Create bootable kernel image (placeholder)
build-image:
    @echo "Creating kernel image..."
    just build-kernel
    @echo "Image creation coming soon"
    # TODO: Implement kernel image creation

# Continuous integration checks
ci: fmt-check lint test-all
    @echo "CI checks passed!"

# Development workflow - format, build, test
dev: fmt build test
    @echo "Development cycle complete!"

# Full verification pipeline
verify: ci check-abi
    @echo "Full verification complete!"

# Benchmark performance
bench:
    cargo bench --workspace

# Code coverage (requires cargo-tarpaulin)
coverage:
    cargo tarpaulin --workspace --out Html --output-dir coverage

# Install development tools
install-tools:
    cargo install cargo-audit
    cargo install cargo-outdated
    cargo install cargo-tarpaulin
    @echo "Development tools installed"

# Phase-specific builds
build-phase-1:
    cargo build -p angzarr-core -p angzarr-list -p angzarr-rbtree -p angzarr-ffi

build-phase-2:
    just build-phase-1
    cargo build -p angzarr-mm

build-phase-3:
    just build-phase-2
    cargo build -p angzarr-sync

# Test specific phase
test-phase-1:
    cargo test -p angzarr-core -p angzarr-list -p angzarr-rbtree -p angzarr-ffi

test-phase-2:
    just test-phase-1
    cargo test -p angzarr-mm

test-phase-3:
    just test-phase-2
    cargo test -p angzarr-sync

# Quick development check
quick: check test
    @echo "Quick check complete!"
