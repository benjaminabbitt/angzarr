# C Reference Implementations

**Purpose**: Standalone C code that mimics Linux kernel data structure behavior for test verification.

---

## Overview

This directory contains **standalone C reference implementations** of Linux kernel data structures and algorithms. Unlike the full Linux kernel source in `tests/linux-kernel/`, these implementations:

✅ Compile in userspace without kernel headers
✅ Are binary-compatible with Linux kernel structures
✅ Can be linked directly into Rust test binaries
✅ Provide deterministic reference behavior for Rust tests

---

## Architecture

```
tests/
├── linux-kernel/          # Full Linux kernel (git submodule)
│   └── Reference only - don't compile directly
│
├── c-reference/           # Standalone C implementations (THIS DIRECTORY)
│   ├── list/              # Doubly-linked list
│   │   ├── list.h         # Header (Linux-compatible)
│   │   ├── list_reference.c  # C implementation
│   │   ├── test_list.c    # C tests
│   │   └── liblist_reference.a  # Compiled library
│   ├── rbtree/            # Red-black tree (future)
│   ├── justfile           # Build commands
│   └── README.md          # This file
│
└── linux-tests/           # Tools for manual testing
    └── justfile           # Build original Linux tests
```

---

## Design Principle

**Problem**: Linux kernel code requires architecture-specific headers (`asm/rwonce.h`, etc.) that don't exist in userspace.

**Solution**: Create **minimal standalone implementations** inspired by Linux but without kernel dependencies.

These implementations:
1. **Match Linux behavior** - Same algorithms, same results
2. **Match Linux layout** - Binary-compatible `#[repr(C)]` structures
3. **Compile in userspace** - No kernel headers needed
4. **Link into Rust tests** - via `build.rs` and `cc` crate

---

## Usage

### Building C Reference Libraries

```bash
cd tests/c-reference

# Build all C reference libraries
just build-all

# Build specific library
just build-list

# Test C reference (verifies C code works)
just test-list

# Clean compiled artifacts
just clean
```

### Using in Rust Tests

C reference libraries are automatically compiled and linked by `build.rs`:

```rust
// angzarr-list/build.rs
fn main() {
    cc::Build::new()
        .file("../tests/c-reference/list/list_reference.c")
        .include("../tests/c-reference/list")
        .compile("list_c_reference");

    // Enable feature flag for tests
    println!("cargo:rustc-cfg=c_reference");
}
```

Then in Rust tests:

```rust
#[cfg(test)]
#[cfg(c_reference)]
mod c_validation {
    extern "C" {
        fn c_ref_list_init(list: *mut list_head);
        fn c_ref_list_add(new: *mut list_head, head: *mut list_head);
        static C_LIST_HEAD_SIZE: usize;
    }

    #[test]
    fn test_rust_matches_c() {
        let mut c_list = list_head::new();
        let mut rust_list = list_head::new();

        unsafe {
            // Get C behavior
            c_ref_list_init(&mut c_list);

            // Get Rust behavior
            rust_list.init();

            // Must match exactly
            assert_eq!(c_list.next, rust_list.next);
            assert_eq!(c_list.prev, rust_list.prev);
        }
    }
}
```

---

## Available Implementations

### List (Doubly-Linked List) ✅

**Status**: Complete and tested

**Files**:
- `list/list.h` - Header with inline functions
- `list/list_reference.c` - Exported functions and constants
- `list/test_list.c` - C tests

**Exported Symbols**:

```c
// Constants
extern const size_t C_LIST_HEAD_SIZE;
extern const size_t C_LIST_HEAD_ALIGN;
extern const size_t C_LIST_HEAD_NEXT_OFFSET;
extern const size_t C_LIST_HEAD_PREV_OFFSET;

// Functions
void c_ref_list_init(struct list_head *list);
void c_ref_list_add(struct list_head *new, struct list_head *head);
void c_ref_list_add_tail(struct list_head *new, struct list_head *head);
void c_ref_list_del(struct list_head *entry);
int c_ref_list_empty(const struct list_head *head);
int c_ref_list_is_head(const struct list_head *list, const struct list_head *head);
int c_ref_list_is_first(const struct list_head *list, const struct list_head *head);
int c_ref_list_is_last(const struct list_head *list, const struct list_head *head);
```

**Verification**:
```bash
cd tests/c-reference
just test-list
# Output:
# ✓ test_init passed
# ✓ test_add passed
# ✓ test_add_tail passed
# ✓ test_del passed
# ✓ test_empty passed
# ✓ test_position passed
# ✓ test_layout passed
# All tests passed! ✓
```

### RBTree (Red-Black Tree) ⏳

**Status**: Not yet implemented

**Planned**: `rbtree/rbtree.h`, `rbtree/rbtree_reference.c`

---

## Creating New C Reference Implementations

### Step 1: Study Linux Implementation

```bash
# View Linux kernel source
cd tests/linux-kernel
cat lib/list.c            # Implementation
cat include/linux/list.h  # Header
cat lib/tests/list-test.c # Tests
```

### Step 2: Extract Minimal Implementation

Create standalone version without kernel dependencies:

```c
/* my_struct.h */
#include <stddef.h>
#include <stdbool.h>

struct my_struct {
    int field1;
    void *field2;
} __attribute__((packed));

void my_function(struct my_struct *s);
```

### Step 3: Implement Reference Functions

```c
/* my_struct_reference.c */
#include "my_struct.h"

const size_t C_MY_STRUCT_SIZE = sizeof(struct my_struct);
const size_t C_MY_STRUCT_ALIGN = __alignof__(struct my_struct);

void c_ref_my_function(struct my_struct *s) {
    my_function(s);
}
```

### Step 4: Add to justfile

```just
build-my-struct:
    {{CC}} {{CFLAGS}} -c my_struct/my_struct_reference.c -o my_struct/my_struct_reference.o
    {{AR}} rcs my_struct/libmy_struct_reference.a my_struct/my_struct_reference.o
    @echo "✓ Built: my_struct/libmy_struct_reference.a"
```

### Step 5: Create build.rs

```rust
// angzarr-my-struct/build.rs
cc::Build::new()
    .file("../tests/c-reference/my_struct/my_struct_reference.c")
    .include("../tests/c-reference/my_struct")
    .compile("my_struct_c_reference");
```

### Step 6: Write Rust Tests

```rust
#[cfg(c_reference)]
mod c_validation {
    extern "C" {
        fn c_ref_my_function(s: *mut my_struct);
    }

    #[test]
    fn test_matches_c() {
        // Test Rust vs C behavior
    }
}
```

---

## Verification Strategy

### 1. C Tests First

Before using as Rust reference, verify C implementation works:

```bash
just test-list
# Must pass all tests
```

### 2. Binary Layout Verification

```bash
just show-sizes
# Check structure sizes match expectations
```

### 3. Rust Integration Test

```bash
cargo test --package angzarr-list
# Tests run with C reference linked
```

### 4. Cross-Validation

Run both C and Rust tests with same inputs, verify same outputs.

---

## Benefits

✅ **Deterministic**: Same compiler, same flags, consistent results
✅ **Automated**: Runs on every `cargo test`
✅ **No kernel headers**: Compiles in userspace
✅ **Binary verification**: Proves memory layout matches
✅ **CI-friendly**: Fast, no external dependencies
✅ **Traceable**: Maps to Linux kernel source

---

## Comparison with Alternatives

| Approach | Pros | Cons | Status |
|----------|------|------|--------|
| **Compile full Linux kernel** | Authentic | Requires kernel headers, arch-specific | ❌ Too complex |
| **Parse Linux test output** | No compilation | Fragile, format-dependent | ❌ Unreliable |
| **AI comparison** | Easy | Non-deterministic | ❌ Not acceptable |
| **Standalone C reference** | Works in userspace, deterministic | Requires maintaining C code | ✅ **CHOSEN** |

---

## Maintenance

### When Linux Kernel Changes

1. Check if changes affect data structure layout
2. Update standalone C implementation if needed
3. Verify tests still pass
4. Update version comment in C files

### Adding New Structures

1. Study Linux implementation
2. Create minimal standalone version
3. Add tests
4. Integrate with Rust

---

## References

- **Linux kernel source**: `tests/linux-kernel/` (git submodule)
- **LINUX_KERNEL_LESSONS.md**: Design Decision #9 (C reference approach)
- **LINUX_TEST_MAPPING.md**: Test traceability
- **.claude.md**: Principle #12 (Test Traceability is King)

---

## License

All C code in this directory is:
- **Inspired by Linux kernel** (GPL-2.0)
- **Binary-compatible with Linux**
- **Licensed under GPL-2.0** (same as Angzarr)

**SPDX-License-Identifier**: GPL-2.0

---

Last updated: 2025-11-16
Implementations: 1 (list) ✅, 0 pending
Tests passing: 100%
