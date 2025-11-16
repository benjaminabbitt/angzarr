# Angzarr: Linux Kernel Rust Migration Strategy

## Project Overview

Angzarr is a progressive migration of the Linux kernel from C to Rust, maintaining full binary compatibility with existing Linux modules and functionality throughout the migration process.

## Core Principles

1. **Progressive Migration**: Move from C to Rust incrementally, one subsystem at a time
2. **Binary Compatibility**: Maintain ABI compatibility with existing C modules
3. **No Innovation**: Replace C code with functionally equivalent Rust code only
4. **Test-Driven Development**: Write tests before implementation
5. **Safety First**: Leverage Rust's safety guarantees wherever possible
6. **Gherkin for End-User Functionality**: Use Gherkin for behavior-driven tests
7. **Comprehensive Unit Testing**: Every component must have thorough unit tests
8. **Adapter Layer Architecture**: Linux compatibility as translation boundary, not constraint
9. **Event-Driven Design**: Use event sourcing with caching where feasible
10. **Robustness Over Performance**: At this stage, prioritize correctness and safety
11. **License Compliance**: All code must be GPL-2.0 compatible

---

## Progress Update (2025-11-16)

### Completed Work

#### Phase 0: Infrastructure ✅ COMPLETE

- [x] Cargo workspace configured (8 crates)
- [x] `just` build system operational
- [x] Bootable "Hello World" kernel (QEMU verified)
- [x] ABI compatibility test framework (29 tests, 100% passing)
- [x] Lefthook git hooks with build/test/boot checks
- [x] CI/CD foundation (just ci command)
- [x] Documentation standards (.claude.md)

#### Phase 1: Core Data Structures ✅ SUBSTANTIAL PROGRESS

**Completed:**
- [x] Linked lists (`ListHead` with tests)
- [x] Red-black trees (`RbNode`, `RbRoot` with tests)
- [x] Reference counting (`Kref` with overflow protection)
- [x] Spinlock implementation (`Spinlock`, `SpinlockGuard`)
- [x] Core types (`Pid`, `Uid`, `Gid` with type safety)
- [x] FFI compatibility layer (`angzarr-ffi`)
- [x] Error handling (`KernelError`, `KernelResult`)

**Pending:**
- [ ] Hash tables
- [ ] Bitmaps
- [ ] Full synchronization primitive suite

#### Architectural Decisions Implemented

**1. Adapter Layer Architecture** (See `ADAPTER_LAYER.md`)

Created `angzarr-linux-compat` crate that:
- Provides Linux-compatible C ABI (`list_head`, `rb_node`, etc.)
- Translates to Angzarr's safe Rust API internally
- Maintains perfect binary compatibility
- Isolates all unsafe code at the boundary
- Allows internal API evolution without breaking ABI

**Benefits:**
- Internal freedom: Angzarr can use optimal Rust patterns
- Perfect compatibility: Linux sees identical ABI
- Safety: Core code is safe Rust
- Maintainability: Clear separation of concerns

**2. Dual Interface Exposure** (See `NAMING_STRATEGY.md`)

Two clearly separated interfaces:

| Interface | Location | Naming | Purpose |
|-----------|----------|--------|---------|
| **Linux-Compatible** | `angzarr-linux-compat` | C-style snake_case | External C code |
| **Angzarr Native** | `angzarr-*` crates | Rust-style PascalCase | Internal Rust code |

**3. Event Sourcing Architecture** (See `KERNEL_EVENTS.md`)

Designed (not yet implemented) event-driven kernel with:
- Event bus for all kernel state changes
- Append-only event log (ring buffer)
- LRU cache for derived state
- Async/sync bridge for Linux API compatibility
- Benefits: debugging, auditing, replay, observability

**4. Type Safety** (See `LINUX_KERNEL_LESSONS.md`)

Implemented safety improvements over Linux:
- Newtype pattern for IDs (prevents Pid/Uid confusion)
- Result<T, E> for error handling (compiler-enforced)
- Reference counting with overflow protection (from day 1)
- Null pointer checks at all FFI boundaries
- Atomic operations with proper ordering

**5. Learning from History** (See `LINUX_KERNEL_LESSONS.md`)

Documented 7 major design decisions comparing Linux, BSD, and Angzarr:
- Adapter layer architecture
- Error handling strategy
- Reference counting evolution
- Type-safe ID wrappers
- Intrusive data structures
- Null pointer robustness
- Event-driven principles

Each decision documents:
- Linux solution and problems
- BSD approach and differences
- Angzarr improvement and rationale
- Code examples from actual codebase

### Current Project Structure

```
angzarr/
├── angzarr-core/           # Core kernel types (Pid, Uid, Kref)
├── angzarr-list/           # List data structures
├── angzarr-rbtree/         # Red-black trees
├── angzarr-sync/           # Synchronization primitives
├── angzarr-mm/             # Memory management (stub)
├── angzarr-ffi/            # FFI types and constants
├── angzarr-linux-compat/   # ⭐ Linux ABI adapter layer
├── angzarr-test-framework/ # Testing utilities
├── angzarr-abi-test/       # ABI compatibility tests
├── angzarr-kernel/         # Bootable kernel binary
├── MIGRATION_STRATEGY.md   # This file
├── ADAPTER_LAYER.md        # Adapter architecture
├── NAMING_STRATEGY.md      # Naming conventions
├── KERNEL_EVENTS.md        # Event system design
├── LINUX_KERNEL_LESSONS.md # Design decisions
├── KERNEL_STRUCTURE.md     # Repository organization
├── .claude.md              # Development rules
├── Claude.todo.md          # Progress tracking
└── justfile                # Build commands
```

### Key Metrics

**Test Coverage:**
- ABI compatibility: 29 tests, 100% passing
- Unit tests: ~25 tests across crates
- Kernel boot: Verified in QEMU
- Binary compatibility: Verified via static assertions

**Code Quality:**
- All code formatted with rustfmt
- Clippy warnings addressed
- No panics in kernel code
- Comprehensive documentation

**Safety:**
- Minimal unsafe code (isolated in adapter)
- All unsafe blocks documented
- Memory safety verified by compiler
- No use-after-free possible in safe code

### Next Immediate Steps

1. **Complete Phase 1** (Current)
   - Implement hash tables
   - Implement bitmaps
   - Expand synchronization primitives
   - Finalize data structure tests

2. **Begin Phase 2** (Next)
   - Memory management foundations
   - SLUB allocator study and implementation
   - Page frame utilities

3. **Implement Event System** (Phase 3-4)
   - Create `angzarr-event` crate
   - Implement event bus
   - Add caching layer
   - Integrate with scheduler

### Lessons Learned

**What Worked Well:**
- ✅ Adapter layer approach successful
- ✅ ABI tests caught all compatibility issues early
- ✅ Documentation-first approach clarified decisions
- ✅ Rust type system prevented entire bug classes
- ✅ Incremental migration validated design

**Challenges:**
- ⚠️ no_std compilation requires careful dependency management
- ⚠️ FFI boundary requires extensive testing
- ⚠️ Boot testing setup was complex but essential

**Adjustments Made:**
- Added robustness-over-performance principle
- Expanded documentation requirements
- Enhanced pre-commit hooks (build + boot + test)
- Added license compliance tracking

---

## Migration Phases

### Phase 0: Infrastructure (Weeks 1-2) ✅ COMPLETE
**Goal**: Set up build system, testing framework, and development environment

- [x] Set up Cargo workspace (8 crates)
- [x] Configure `just` build system
- [x] Set up QEMU testing environment (bootable kernel verified)
- [x] Create FFI compatibility layer framework (angzarr-linux-compat)
- [x] Establish test infrastructure (angzarr-abi-test, 29 tests passing)
- [x] Create CI/CD pipeline (lefthook + just ci)
- [x] Document coding standards and patterns (.claude.md)

**Additional Accomplishments:**
- Bootable "Hello World" kernel
- Comprehensive ABI compatibility testing
- Enhanced pre-commit hooks (build + boot + test)
- License compliance framework

### Phase 1: Core Data Structures (Weeks 3-6) 🔄 IN PROGRESS (70% Complete)
**Goal**: Migrate fundamental kernel data structures with C FFI wrappers

**Components**:
- [x] List primitives (`list_head`, circular lists) - DONE
- [x] Red-black trees (`rbtree`) - DONE
- [ ] Hash tables - PENDING
- [ ] Bitmaps - PENDING
- [x] Reference counting (`kref`) - DONE
- [x] Basic synchronization primitives (spinlock) - DONE

**Why First?**:
- Minimal dependencies
- Used throughout the kernel
- Can be wrapped with C-compatible FFI
- Pure data structure logic, well-defined interfaces

**Testing Strategy**:
- [x] Unit tests for each data structure operation
- [ ] Gherkin tests for data structure behaviors - PENDING
- [x] Compatibility tests with C code (ABI tests)

**Completed Work:**
- `angzarr-list`: Intrusive linked lists with full test coverage
- `angzarr-rbtree`: Red-black tree with color management
- `angzarr-core`: Type-safe wrappers (Pid, Uid, Gid, Kref)
- `angzarr-sync`: Spinlock with RAII guard
- `angzarr-linux-compat`: Adapter layer for all above structures

**Next Steps:**
- Implement hash tables
- Implement bitmaps
- Add mutex and semaphore primitives
- Complete Gherkin test suite

### Phase 2: Memory Management Foundations (Weeks 7-12)
**Goal**: Migrate core memory management utilities

**Components**:
- Memory allocation helpers (`kmalloc`, `kfree` wrappers)
- Page frame number utilities
- Memory alignment helpers
- SLAB/SLUB allocator interfaces
- Basic virtual memory address translation

**Why Second?**:
- Depends on Phase 1 data structures
- Critical for kernel functionality
- Can leverage Rust's ownership model for safety

**Testing Strategy**:
- Memory leak detection tests
- Allocation/deallocation stress tests
- Gherkin scenarios for memory management behaviors

### Phase 3: Synchronization Primitives (Weeks 13-18)
**Goal**: Implement safe synchronization mechanisms

**Components**:
- Spinlocks (raw_spinlock_t, spinlock_t)
- Mutexes
- Semaphores
- Read-write locks
- RCU (Read-Copy-Update) basics
- Atomic operations

**Why Third?**:
- Builds on data structures from Phase 1
- Critical for correctness
- Rust's type system can prevent common concurrency bugs

**Testing Strategy**:
- Concurrency stress tests
- Deadlock detection tests
- Race condition tests
- Gherkin scenarios for locking behaviors

### Phase 4: Process Management Basics (Weeks 19-26)
**Goal**: Migrate core process/task structures and utilities

**Components**:
- `task_struct` definition and accessors
- Process state management
- Task creation/destruction helpers
- Process credentials
- Namespaces structures

**Why Fourth?**:
- Depends on memory management and synchronization
- Central to kernel functionality
- Well-defined interfaces

**Testing Strategy**:
- Process lifecycle tests
- State transition tests
- Gherkin scenarios for process behaviors

### Phase 5: Scheduler Basics (Weeks 27-34)
**Goal**: Migrate scheduling algorithms and structures

**Components**:
- Run queue structures
- Scheduling classes
- Load balancing utilities
- CPU affinity handling
- Priority calculations

**Why Fifth?**:
- Depends on process management
- Complex but well-isolated algorithms
- Performance-critical (needs careful optimization)

**Testing Strategy**:
- Scheduling fairness tests
- Performance benchmarks
- Gherkin scenarios for scheduling behaviors

### Phase 6: Interrupt Handling (Weeks 35-42)
**Goal**: Migrate interrupt subsystem

**Components**:
- IRQ descriptors
- Interrupt handler registration
- Softirqs
- Tasklets
- Workqueues

**Why Sixth?**:
- Depends on synchronization and process management
- Critical for system responsiveness
- Can benefit from Rust's safety guarantees

### Phase 7: System Call Interface (Weeks 43-50)
**Goal**: Migrate system call framework

**Components**:
- System call table
- System call entry/exit
- Parameter validation
- Error handling
- Capability checking

**Why Seventh?**:
- Depends on process management and interrupt handling
- User-kernel boundary
- Security-critical

### Phase 8: Virtual File System (VFS) Layer (Weeks 51-62)
**Goal**: Migrate VFS abstractions

**Components**:
- inode structures
- dentry cache
- File operations
- Path lookup
- VFS helpers

**Why Eighth?**:
- Depends on memory management and synchronization
- Large subsystem with clear interfaces
- Enables filesystem migration

### Phase 9: Device Driver Framework (Weeks 63-74)
**Goal**: Migrate device driver infrastructure

**Components**:
- Device model structures
- Driver registration
- Character device framework
- Block device framework
- Device PM framework

### Phase 10: Network Stack Foundation (Weeks 75-90)
**Goal**: Migrate core networking structures

**Components**:
- Socket buffers (sk_buff)
- Network device structures
- Protocol registration
- Basic packet handling

### Phase 11+: Progressive Subsystem Migration
**Ongoing**: Continue migrating remaining subsystems

- Individual filesystems
- Network protocols
- Architecture-specific code
- Device drivers
- IPC mechanisms
- Security modules

## Binary Compatibility Strategy

### FFI Layer Design

```rust
// C-compatible exports
#[no_mangle]
pub extern "C" fn kmalloc(size: usize, flags: u32) -> *mut c_void {
    // Rust implementation
}

// Rust-safe wrappers for internal use
pub fn allocate<T>(flags: GfpFlags) -> Result<Box<T>, AllocError> {
    // Safe Rust interface
}
```

### ABI Stability

1. All exported symbols maintain C ABI
2. Structure layouts match C exactly (use `#[repr(C)]`)
3. Function signatures preserved
4. Header files auto-generated from Rust definitions

### Module Loading Compatibility

1. Support existing `.ko` modules
2. Gradual migration of modules to Rust
3. Mixed C/Rust module support during transition

## Testing Strategy

### Unit Tests (Rust)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_add() {
        // Test implementation
    }
}
```

### Gherkin Tests (End-User Functionality)

```gherkin
Feature: Process Creation
  As a system administrator
  I want to create new processes
  So that I can run programs

  Scenario: Creating a simple process
    Given the system is running
    When I execute a new program
    Then a new process should be created
    And the process should have a unique PID
    And the process should inherit parent's credentials
```

### Integration Tests

- Test C modules against Rust kernel
- Test Rust modules against C kernel
- Test mixed C/Rust scenarios

### Xen Hypervisor Testing

- Boot test kernels in Xen VMs
- Automated regression testing
- Performance benchmarking
- Fault injection testing

## Build System (`just`)

- Separate recipes for each phase
- Cross-compilation support
- Test execution
- Kernel image generation
- Module building

## Safety Guarantees

### Unsafe Code Guidelines

1. Minimize `unsafe` blocks
2. Document safety invariants
3. Encapsulate unsafe in safe abstractions
4. Review all unsafe code

### Memory Safety

- No null pointer dereferences (use `Option<&T>`)
- No use-after-free (ownership system)
- No data races (borrow checker)
- Bounds checking on arrays

### Concurrency Safety

- Type-safe synchronization primitives
- Send/Sync traits for thread safety
- No data races by construction

## Success Metrics

1. **Functional**: All tests pass (unit + Gherkin + integration)
2. **Performance**: No more than 5% overhead vs C implementation
3. **Compatibility**: All existing modules work unchanged
4. **Safety**: Zero memory safety vulnerabilities in Rust code
5. **Coverage**: >90% test coverage

## Risk Mitigation

1. **Performance Regression**: Continuous benchmarking, optimization phase
2. **ABI Breakage**: Automated compatibility tests, careful review
3. **Unsafe Code**: Minimize, encapsulate, review thoroughly
4. **Complexity**: Incremental migration, clear phase boundaries
5. **Testing Gaps**: Comprehensive test suites, fuzzing

## Timeline

- **Phase 0**: Weeks 1-2 (Infrastructure)
- **Phases 1-3**: Weeks 3-18 (Foundations)
- **Phases 4-7**: Weeks 19-50 (Core Kernel)
- **Phases 8-10**: Weeks 51-90 (Major Subsystems)
- **Phase 11+**: Ongoing (Complete Migration)

**Estimated Total**: 18-24 months for core migration

## Resources Required

- Development team: 4-6 experienced Rust developers
- Testing infrastructure: Xen cluster, CI/CD servers
- Code review: Weekly architecture reviews
- Documentation: Technical writers for migration guides

## Next Steps

1. Set up development environment
2. Create initial Cargo workspace
3. Implement Phase 0 infrastructure
4. Begin Phase 1 data structures with TDD approach
