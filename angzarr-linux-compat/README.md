# Angzarr Linux Compatibility Layer

## Purpose

This crate provides a **boundary/adapter layer** that translates between:
- **Linux Kernel C ABI** (external interface)
- **Angzarr Rust API** (internal implementation)

This design keeps Angzarr's core free from Linux-specific constraints while maintaining perfect binary compatibility.

## Architecture

```
┌────────────────────────────────────┐
│   Linux C Code / Modules           │
│   Uses: struct list_head, etc.     │
└─────────────┬──────────────────────┘
              │
              │ C ABI (#[no_mangle], extern "C")
              │
┌─────────────▼──────────────────────┐
│ angzarr-linux-compat (THIS CRATE)  │
│ • Translates C ↔ Rust              │
│ • Maintains #[repr(C)] structs     │
│ • Converts errors to errno         │
│ • Zero-cost abstraction            │
└─────────────┬──────────────────────┘
              │
              │ Safe Rust API
              │
┌─────────────▼──────────────────────┐
│ Angzarr Core                       │
│ • Pure Rust implementation         │
│ • Safe abstractions                │
│ • No Linux constraints             │
└────────────────────────────────────┘
```

## Modules

### `list` - Linked Lists
- Provides `struct list_head` C ABI
- Functions: `INIT_LIST_HEAD`, `list_add`, `list_del`, etc.
- Translates to Angzarr's safe list implementation

### `rbtree` - Red-Black Trees
- Provides `struct rb_node` and `struct rb_root` C ABI
- Functions: `rb_insert`, `rb_erase`, etc.
- Uses Angzarr's type-safe rbtree internally

### `error` - Error Translation
- Converts Rust `Result` to Linux errno values
- Bidirectional error code translation
- Zero overhead

### `types` - Type Aliases
- Linux types: `pid_t`, `uid_t`, `gid_t`
- Maps to Angzarr's safe wrapper types

## Usage from C

```c
#include "linux/list.h"

struct my_data {
    int value;
    struct list_head list;
};

void example(void) {
    struct list_head head;
    INIT_LIST_HEAD(&head);

    struct my_data *item = kmalloc(sizeof(*item), GFP_KERNEL);
    item->value = 42;
    list_add(&item->list, &head);

    // Works exactly like Linux kernel
}
```

## Usage from Rust

Internal Angzarr code uses the safe Rust API:

```rust
use angzarr_list::List;

fn example() {
    let mut list = List::new();
    list.push_front(42);
    assert_eq!(list.pop_front(), Some(42));

    // Safe, ergonomic Rust API
}
```

## Design Principles

1. **No Business Logic**: This crate only translates, never implements
2. **Perfect ABI Match**: Binary layout must match Linux exactly
3. **Zero Cost**: Translation should compile to identical code
4. **Safety Boundary**: All unsafe isolated here, core is safe
5. **Testable**: Both APIs tested independently

## Testing

### ABI Compatibility Tests

Verify C structures match Linux:
```bash
cargo test -p angzarr-abi-test
```

### Adapter Tests

Verify translation correctness:
```bash
cargo test -p angzarr-linux-compat
```

## Adding New APIs

When adding a new Linux-compatible API:

1. **Define C-compatible struct** with `#[repr(C)]`
2. **Export functions** with `#[no_mangle]` and `extern "C"`
3. **Translate** to/from Angzarr's safe API
4. **Add tests** for both C and Rust interfaces
5. **Update ABI tests** to verify binary compatibility

Example:
```rust
// 1. C-compatible struct
#[repr(C)]
pub struct my_linux_type {
    pub field: *mut c_void,
}

// 2. Export function
#[no_mangle]
pub unsafe extern "C" fn my_linux_function(ptr: *mut my_linux_type) {
    if ptr.is_null() {
        return;
    }

    // 3. Translate to Angzarr API
    let rust_value = angzarr_core::safe_function(/* ... */);

    // 4. Convert back to C
    (*ptr).field = rust_value.as_ptr();
}
```

## Benefits

✅ **Internal Freedom**: Angzarr can use optimal Rust patterns
✅ **Perfect Compatibility**: Linux sees identical ABI
✅ **Safety**: Core code is safe Rust
✅ **Maintainability**: Clear separation of concerns
✅ **Evolution**: Can improve internals without breaking ABI

## References

- [ADAPTER_LAYER.md](../ADAPTER_LAYER.md) - Full architecture documentation
- [MIGRATION_STRATEGY.md](../MIGRATION_STRATEGY.md) - Migration plan
- [ABI_TEST_RESULTS.md](../ABI_TEST_RESULTS.md) - Compatibility verification

## Status

**Current:** Foundation implemented
- List adapter: ✅ Complete
- RBTree adapter: ✅ Basic
- Error translation: ✅ Complete
- Types: ✅ Complete

**Next:** Expand as more subsystems are migrated
