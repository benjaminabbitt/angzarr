# &angzarr; Angzarr

**A Progressive Rust Migration of the Linux Kernel**

Angzarr is a progressive, phase-by-phase migration of the Linux kernel from C to Rust, maintaining full binary compatibility with existing Linux modules throughout the migration process.

## Project Goals

- **Progressive Migration**: Incrementally replace C code with Rust, one subsystem at a time
- **Binary Compatibility**: Maintain ABI compatibility with existing kernel modules
- **No Innovation**: Focus solely on replacing C with functionally equivalent Rust code
- **Maximum Safety**: Leverage Rust's memory safety and concurrency guarantees
- **Test-Driven Development**: Comprehensive testing with unit tests and Gherkin scenarios
- **Production Ready**: Build a kernel suitable for real-world deployment

## Architecture

### Workspace Structure

```
angzarr/
├── angzarr-core/          # Core kernel types (Pid, Uid, Kref, etc.)
├── angzarr-ffi/           # C FFI compatibility layer
├── angzarr-list/          # Intrusive doubly-linked lists
├── angzarr-rbtree/        # Red-black trees
├── angzarr-sync/          # Synchronization primitives (spinlocks, mutexes)
├── angzarr-mm/            # Memory management
├── angzarr-test-framework/# Gherkin/Cucumber testing framework
├── xen-test/              # Xen hypervisor test configurations
└── scripts/               # Build and test automation scripts
```

## Migration Strategy

The migration follows a carefully planned, multi-phase approach:

### Phase 0: Infrastructure ✅
- Cargo workspace setup
- `just` build system
- Testing framework (unit tests + Gherkin)
- Xen testing environment
- CI/CD pipeline

### Phase 1: Core Data Structures ✅
- Linked lists (`list_head`)
- Red-black trees (`rbtree`)
- Hash tables
- Reference counting (`kref`)
- Basic synchronization types

### Phase 2: Memory Management (In Progress)
- Memory allocation (`kmalloc`, `kfree`)
- Page allocators
- SLAB/SLUB allocator
- Virtual memory utilities

### Phase 3: Synchronization Primitives (Planned)
- Spinlocks
- Mutexes
- Semaphores
- RCU (Read-Copy-Update)
- Atomic operations

### Future Phases
- Process management
- Scheduler
- Interrupt handling
- System calls
- Virtual File System
- Device drivers
- Network stack

See [MIGRATION_STRATEGY.md](MIGRATION_STRATEGY.md) for detailed phase breakdown.

## Building

### Prerequisites

- Rust toolchain (stable or nightly)
- `just` command runner: `cargo install just`
- (Optional) `cargo-audit`, `cargo-outdated`, `cargo-tarpaulin` for development

### Quick Start

```bash
# Build the entire workspace
just build

# Run all tests
just test

# Run linter
just lint

# Format code
just fmt

# Full CI verification
just ci
```

### Phase-Specific Builds

```bash
# Build Phase 1 components only
just build-phase-1

# Test Phase 1 components
just test-phase-1

# Build with kernel optimizations
just build-kernel
```

## Testing

Angzarr uses a comprehensive testing strategy:

### Unit Tests

Every module includes extensive unit tests:

```bash
# Run all unit tests
just test

# Run tests for specific crate
just test-crate angzarr-list

# Verbose test output
just test-verbose
```

### Gherkin/BDD Tests

End-user functionality is tested using Gherkin scenarios:

```bash
# Run Gherkin tests
just test-gherkin
```

Example Gherkin test:

```gherkin
Feature: Kernel Linked List
  Scenario: Adding entries to a list
    Given an empty list
    When I add an entry to the list
    Then the list should not be empty
```

### Property-Based Testing

Critical data structures use property-based testing with proptest:

```bash
just test-prop
```

### Xen Hypervisor Testing

Test kernels boot in Xen VMs for integration testing:

```bash
# Run Xen tests (when kernel is bootable)
just test-xen

# Or use the script directly
./scripts/test-xen.sh
```

## Development Workflow

### TDD Cycle

1. Write a failing test
2. Implement minimal code to pass the test
3. Refactor while keeping tests green
4. Commit

```bash
# Quick development cycle
just dev
```

### Safety Guidelines

- Minimize `unsafe` blocks
- Document all safety invariants
- Encapsulate `unsafe` in safe abstractions
- Use `#[deny(unsafe_op_in_unsafe_fn)]`
- Review all unsafe code

### Code Style

- Follow Rust API guidelines
- Use `rustfmt` for formatting
- Run `clippy` for lints
- Document all public APIs
- Add examples to documentation

## Binary Compatibility

### C ABI Compatibility

All exported functions maintain C ABI:

```rust
#[no_mangle]
pub extern "C" fn list_add(new: *mut ListHead, head: *mut ListHead) {
    unsafe { (*head).add(new) }
}
```

### Structure Layout

All kernel structures use `#[repr(C)]`:

```rust
#[repr(C)]
pub struct ListHead {
    pub next: *mut ListHead,
    pub prev: *mut ListHead,
}
```

### FFI Layer

The `angzarr-ffi` crate provides C-compatible types and error codes:

```rust
use angzarr_ffi::{GfpFlags, KernelError, KernelResult};
```

## Performance

Performance is critical for kernel code:

- Release builds use LTO and single codegen unit
- `kernel` profile provides maximum optimization
- Benchmarking suite for critical paths
- Performance regression testing in CI

```bash
# Run benchmarks
just bench
```

## Documentation

### Generate Documentation

```bash
# Build and open documentation
just doc
```

### Key Documents

- [MIGRATION_STRATEGY.md](MIGRATION_STRATEGY.md) - Detailed migration plan
- [Cargo.toml](Cargo.toml) - Workspace configuration
- [justfile](justfile) - Build commands reference

## Contributing

### Development Setup

```bash
# Clone repository
git clone https://github.com/benjaminabbitt/angzarr.git
cd angzarr

# Install development tools
just install-tools

# Run full verification
just verify
```

### Code Review Process

1. All code must pass CI (`just ci`)
2. Unsafe code requires thorough review
3. All features must have tests
4. Documentation must be updated

## Project Status

### Current Phase: Phase 1 (Core Data Structures)

**Completed:**
- ✅ Project infrastructure
- ✅ Build system (`just`)
- ✅ Testing framework
- ✅ FFI layer
- ✅ Core types (`Kref`, `Pid`, `Uid`)
- ✅ Linked lists with unit tests
- ✅ Red-black trees with unit tests
- ✅ Spinlocks (basic implementation)

**In Progress:**
- 🔄 Memory management foundations
- 🔄 Gherkin test implementation
- 🔄 Xen testing infrastructure

**Next Steps:**
- Memory allocator implementation
- Complete synchronization primitives
- Process management structures

## Benchmarks

Coming soon - performance comparisons with C implementation.

## License

GPL-2.0 (matching Linux kernel license)

## Acknowledgments

- Linux kernel developers for the original implementation
- Rust for Linux project for pioneering Rust in kernel development
- Rust community for excellent tooling and libraries

## Resources

- [Linux Kernel Documentation](https://www.kernel.org/doc/)
- [Rust for Linux](https://rust-for-linux.com/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

## Contact

- Repository: https://github.com/benjaminabbitt/angzarr
- Issues: https://github.com/benjaminabbitt/angzarr/issues

---

**Note**: Angzarr is a research and development project. It is not yet production-ready and should not be used in critical systems.
