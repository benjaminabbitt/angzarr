# Angzarr Kernel Structure

## Linux Kernel Compatibility

Angzarr follows the Linux kernel directory structure and organization while applying Rust conventions where appropriate. This document outlines how Angzarr maintains binary compatibility and organizational alignment with the Linux kernel.

## Directory Structure Mapping

### Linux Kernel → Angzarr Mapping

```
linux/                          angzarr/
├── kernel/                     ├── angzarr-core/         (core kernel functionality)
│   ├── sched.c                 │   └── src/scheduler.rs
│   ├── fork.c                  │   └── src/process.rs
│   └── ...                     │   └── src/types.rs
├── mm/                         ├── angzarr-mm/           (memory management)
│   ├── slab.c                  │   └── src/allocator.rs
│   ├── page_alloc.c            │   └── src/page.rs
│   └── ...                     │   └── src/slab.rs
├── fs/                         ├── angzarr-fs/           (filesystem)
│   ├── namei.c                 │   └── src/namei.rs
│   ├── inode.c                 │   └── src/inode.rs
│   └── ...                     │   └── src/vfs.rs
├── net/                        ├── angzarr-net/          (networking)
│   ├── socket.c                │   └── src/socket.rs
│   └── ...                     │   └── src/stack.rs
├── drivers/                    ├── angzarr-drivers/      (device drivers)
│   ├── char/                   │   ├── char/
│   ├── block/                  │   ├── block/
│   └── net/                    │   └── net/
├── lib/                        ├── angzarr-list/         (core data structures)
│   ├── rbtree.c                ├── angzarr-rbtree/
│   ├── list.c                  └── angzarr-sync/         (locking primitives)
│   └── ...
├── include/                    └── angzarr-ffi/          (C compatibility headers)
│   ├── linux/                      └── src/lib.rs       (FFI definitions)
│   └── ...
└── arch/                       └── angzarr-arch/         (architecture-specific)
    ├── x86/                        ├── x86_64/
    └── ...                         └── aarch64/
```

## Organizational Principles

### 1. Crate Organization

Each major Linux subsystem maps to a Rust crate:

- **angzarr-core**: Core kernel (kernel/, init/)
- **angzarr-mm**: Memory management (mm/)
- **angzarr-fs**: Filesystems (fs/)
- **angzarr-net**: Networking (net/)
- **angzarr-drivers**: Device drivers (drivers/)
- **angzarr-list**: Intrusive data structures (lib/list.c, lib/rbtree.c)
- **angzarr-rbtree**: Red-black trees (lib/rbtree.c)
- **angzarr-sync**: Synchronization (include/linux/spinlock.h, etc.)
- **angzarr-ffi**: C FFI layer (include/linux/)

### 2. Module Boundaries

Crates follow Linux subsystem boundaries:

```rust
// angzarr-mm/src/lib.rs
pub mod allocator;    // mm/slab.c, mm/slub.c
pub mod page;         // mm/page_alloc.c
pub mod vmalloc;      // mm/vmalloc.c
pub mod mmap;         // mm/mmap.c
```

### 3. File-Level Mapping

Individual C source files map to Rust modules:

```
Linux: mm/slab.c          → Rust: angzarr-mm/src/slab.rs
Linux: kernel/sched/core.c → Rust: angzarr-core/src/scheduler/core.rs
Linux: fs/namei.c         → Rust: angzarr-fs/src/namei.rs
```

## Binary Compatibility

### C ABI Compliance

All exported symbols maintain C ABI compatibility:

```rust
// Linux: list_add() in include/linux/list.h
#[no_mangle]
pub extern "C" fn list_add(new: *mut ListHead, head: *mut ListHead) {
    unsafe { (*head).add(new) }
}
```

### Structure Layout

All kernel structures use `#[repr(C)]` to match Linux layout:

```rust
// Linux: struct list_head
#[repr(C)]
pub struct ListHead {
    pub next: *mut ListHead,    // Matches Linux exactly
    pub prev: *mut ListHead,
}
```

### Header File Generation

C headers are auto-generated from Rust for compatibility:

```
angzarr-ffi/include/
├── angzarr/
│   ├── list.h          (generated from angzarr-list)
│   ├── rbtree.h        (generated from angzarr-rbtree)
│   └── types.h         (generated from angzarr-core)
```

## Code Organization

### Subsystem Structure

Each crate follows this internal structure:

```
angzarr-{subsystem}/
├── Cargo.toml
├── src/
│   ├── lib.rs          (Public API, matches Linux subsystem interface)
│   ├── internal.rs     (Internal helpers)
│   └── {component}.rs  (Individual components from Linux)
└── tests/
    └── integration.rs  (Subsystem tests)
```

### Example: Memory Management

```rust
// angzarr-mm/src/lib.rs
#![cfg_attr(not(test), no_std)]

// Public API matching Linux mm/
pub mod allocator;      // kmalloc, kfree (mm/slab.c)
pub mod page;           // page allocator (mm/page_alloc.c)
pub mod vmalloc;        // vmalloc (mm/vmalloc.c)

// Internal modules
mod internal;

// Re-exports matching Linux API
pub use allocator::{kmalloc, kfree};
pub use page::{alloc_pages, free_pages};
```

## Function Naming Conventions

### Rust Internal API

Follow Rust conventions:

```rust
impl ListHead {
    pub fn is_empty(&self) -> bool { ... }    // Rust-style
    pub unsafe fn add(&mut self, ...) { ... }
}
```

### C FFI Exports

Match Linux naming exactly:

```rust
#[no_mangle]
pub extern "C" fn list_empty(head: *const ListHead) -> bool { ... }

#[no_mangle]
pub extern "C" fn list_add(new: *mut ListHead, head: *mut ListHead) { ... }
```

## Data Structure Alignment

### Linux List Example

Linux (include/linux/list.h):
```c
struct list_head {
    struct list_head *next, *prev;
};
```

Angzarr (angzarr-list/src/lib.rs):
```rust
#[repr(C)]
pub struct ListHead {
    pub next: *mut ListHead,
    pub prev: *mut ListHead,
}
```

**Binary Layout**: Identical - verified by static_assertions

## Subsystem Dependencies

Follow Linux dependency hierarchy:

```
angzarr-core
├── angzarr-ffi       (types, error codes)
└── angzarr-list      (data structures)

angzarr-mm
├── angzarr-core
├── angzarr-sync      (locks)
└── angzarr-list      (free lists)

angzarr-fs
├── angzarr-core
├── angzarr-mm
└── angzarr-sync
```

## Build System Alignment

### Kconfig → Cargo Features

Linux Kconfig options map to Cargo features:

```toml
[features]
default = []
smp = []              # CONFIG_SMP
debug = []            # CONFIG_DEBUG_KERNEL
tracing = []          # CONFIG_TRACING
```

### Makefile → Justfile

Build commands maintain similar structure:

```bash
# Linux
make vmlinux
make modules

# Angzarr
just build-kernel
just build-modules
```

## Documentation Standards

### Kernel Doc → Rust Doc

Linux kernel-doc comments convert to Rust doc comments:

```rust
/// Allocate kernel memory
///
/// # Arguments
/// * `size` - Number of bytes to allocate
/// * `flags` - GFP allocation flags
///
/// # Safety
/// Caller must ensure proper deallocation with kfree
///
/// # Returns
/// Pointer to allocated memory or NULL on failure
#[no_mangle]
pub unsafe extern "C" fn kmalloc(size: usize, flags: u32) -> *mut u8 {
    ...
}
```

## Testing Strategy

### Unit Tests

Each module has unit tests matching Linux behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_add() {
        // Test behavior matches Linux implementation
    }
}
```

### Integration Tests

Subsystem tests verify Linux API compatibility:

```rust
// tests/compat.rs
#[test]
fn test_linux_list_api() {
    // Verify C API compatibility
}
```

## Migration Path

### Phase-by-Phase Conversion

1. **Phase 1**: Core data structures (lib/)
2. **Phase 2**: Memory management (mm/)
3. **Phase 3**: Process management (kernel/)
4. **Phase 4**: Scheduling (kernel/sched/)
5. **Phase 5**: Filesystems (fs/)
6. **Phase 6**: Networking (net/)
7. **Phase 7**: Drivers (drivers/)

### API Stability

- C ABI: **Never breaks** - 100% compatible
- Rust API: **May evolve** - internal only
- Module interface: **Follows Linux** - stable

## Verification

### ABI Compatibility Tests

```bash
# Verify structure sizes match Linux
just check-abi

# Test C module loading
just test-module-compat
```

### Size Verification

```rust
use static_assertions::assert_eq_size;

assert_eq_size!(ListHead, [usize; 2]);  // Same as Linux
```

## References

- Linux Kernel Source: https://kernel.org/
- Linux Documentation: Documentation/
- Rust for Linux: https://rust-for-linux.com/

## Compliance Checklist

- [ ] Directory structure mirrors Linux subsystems
- [ ] File organization matches Linux source files
- [ ] C ABI compatibility maintained
- [ ] Structure layouts verified identical
- [ ] Function naming matches Linux exports
- [ ] Dependencies follow Linux hierarchy
- [ ] Documentation matches Linux standards
- [ ] Tests verify Linux behavior
- [ ] Binary compatibility verified

---

**Note**: Angzarr maintains strict Linux kernel organizational compatibility while leveraging Rust's safety features. When Rust conventions conflict with Linux structure, we preserve Linux organization and add Rust safety on top through careful API design.
