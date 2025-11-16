# Linux Kernel Test Comparison Infrastructure

**Purpose**: Reference Linux kernel source via git submodule and compile tests for comparison with Angzarr Rust implementations.

---

## Overview

This directory provides infrastructure to:

1. **Reference** Linux kernel source via git submodule
2. **Compile** C tests from kernel for userspace execution
3. **Compare** C test results with Rust implementation results
4. **Verify** binary-level compatibility

This ensures our Rust implementations produce **identical** results to Linux.

---

## Quick Start

```bash
# 1. Initialize Linux kernel submodule (first time only)
cd ../..
git submodule update --init --recursive

# 2. Build C test from kernel source
cd tests/linux-tests
just build-list

# 3. Run C test
just run-list

# 4. Compare with Rust implementation
just verify-list
```

---

## Architecture

### Two Verification Approaches

**Approach 1: Manual Comparison** (this directory)
- Compile C tests standalone
- Run them manually
- Compare output with Rust tests
- Good for debugging and development

**Approach 2: Automated Comparison** (in Rust crates)
- Compile C code into Rust test binary via `build.rs`
- Call C functions/access C variables directly from Rust tests
- Automatic verification on every `cargo test`
- **Preferred approach** - see LINUX_KERNEL_LESSONS.md Decision #9

---

## Linux Kernel Submodule

### Location

```
tests/
├── linux-kernel/          # Git submodule: Full Linux kernel source
│   ├── lib/
│   │   ├── test_list.c   # List tests
│   │   ├── rbtree_test.c # RBTree tests
│   │   ├── test_bitmap.c # Bitmap tests
│   │   └── ...
│   ├── include/linux/
│   │   ├── list.h
│   │   ├── rbtree.h
│   │   └── ...
│   └── ...
└── linux-tests/           # This directory
    ├── justfile           # Build commands
    └── README.md          # This file
```

### Initialization

First time setup:

```bash
# From repository root
git submodule add https://github.com/torvalds/linux.git tests/linux-kernel
git submodule update --init --recursive
```

Update to newer kernel version:

```bash
cd tests/linux-kernel
git fetch
git checkout v6.8  # Or desired version
cd ../..
git add tests/linux-kernel
git commit -m "Update Linux kernel submodule to v6.8"
```

Check current version:

```bash
just kernel-version
```

---

## Using `just` for Build Tasks

### Design Decision

We use [`just`](https://github.com/casey/just) instead of `make` for all build tasks.

**Rationale:**
- ✅ Consistent with main Angzarr build system
- ✅ Clear, readable syntax (no tab/space issues)
- ✅ Cross-platform (works on Linux, macOS, Windows)
- ✅ Better error messages
- ✅ Modern features (dependencies, arguments, variables)

See **LINUX_KERNEL_LESSONS.md Decision #8** for full rationale.

### Available Commands

```bash
# List all available commands
just --list

# Initialize submodule
just init

# Build specific test
just build-list
just build-rbtree
just build-bitmap
just build-hash

# Build all tests
just build-all

# Run specific test
just run-list
just run-rbtree

# Run all tests
just run-all

# Verify Rust matches C
just verify-list
just verify-rbtree

# Browse available tests in kernel
just browse-tests

# Check kernel version
just kernel-version

# Clean compiled tests
just clean
just distclean
```

---

## Test Compilation

### How It Works

1. **Source**: C test files from `tests/linux-kernel/lib/`
2. **Compiler**: GCC with `-Wall -Wextra` warnings
3. **Includes**: Linux kernel headers from `tests/linux-kernel/include/`
4. **Stubs**: Minimal stubs for userspace compilation
5. **Output**: Executable test binaries

### Example: List Tests

```bash
# Build list tests
just build-list

# Internally runs:
# gcc -Wall -Wextra -g -O2 \
#   -I../linux-kernel/include -I. \
#   -o test-list \
#   ../linux-kernel/lib/test_list.c
```

### Limitations

Some kernel tests cannot run in userspace:
- Tests requiring kernel memory management
- Tests requiring hardware access
- Tests depending on kernel modules

We adapt tests where needed or focus on unit tests that can run standalone.

---

## Verification Workflow

### Manual Verification (For Debugging)

```bash
# 1. Build C test
just build-list

# 2. Run C test and save output
./test-list > list_c_output.txt 2>&1

# 3. Run Rust test and save output
cd ../..
cargo test --package angzarr-list -- --nocapture > tests/linux-tests/list_rust_output.txt 2>&1

# 4. Compare outputs
diff tests/linux-tests/list_c_output.txt tests/linux-tests/list_rust_output.txt

# Should show NO differences (or only formatting differences)
```

### Automated Verification (Recommended)

Use `just verify-*` commands:

```bash
# Runs C test, Rust test, and compares automatically
just verify-list
just verify-rbtree
```

### Best Approach: Direct C Integration

**Recommended**: Compile C code directly into Rust test binaries.

See **LINUX_KERNEL_LESSONS.md Decision #9** for full details.

Example `build.rs`:

```rust
// angzarr-list/build.rs
use std::path::PathBuf;

fn main() {
    let kernel = PathBuf::from("../tests/linux-kernel");

    if !kernel.exists() {
        eprintln!("Warning: Linux kernel submodule not initialized");
        return;
    }

    // Compile C reference code
    cc::Build::new()
        .file(kernel.join("lib/list_sort.c"))
        .include(kernel.join("include"))
        .warnings(false)
        .compile("list_c_ref");
}
```

Then in tests:

```rust
#[cfg(test)]
mod c_reference {
    extern "C" {
        fn c_list_add(new: *mut list_head, head: *mut list_head);
        static EXPECTED_SIZE: usize;
    }
}

#[test]
fn test_matches_c() {
    // Call C function or access C variable
    unsafe {
        let expected = c_reference::EXPECTED_SIZE;
        assert_eq!(size_of::<list_head>(), expected);
    }
}
```

---

## Available Tests

| Test | Linux Source | Status | Notes |
|------|--------------|--------|-------|
| **list** | `lib/test_list.c` | ✅ | Linked list operations |
| **rbtree** | `lib/rbtree_test.c` | ✅ | Red-black tree |
| **bitmap** | `lib/test_bitmap.c` | ⚠️ | May need adaptation |
| **hash** | `lib/test_hash.c` | ⚠️ | May need adaptation |

---

## Troubleshooting

### Submodule Not Initialized

```
Error: Linux kernel submodule not initialized
```

**Solution:**
```bash
cd ../..
git submodule update --init --recursive
```

### Compilation Errors

```
error: undefined reference to ...
```

**Solutions:**
1. Add missing stubs in `linux_types.h`
2. Provide userspace equivalents of kernel functions
3. Use `#ifdef` to conditionally compile kernel-only code
4. Some tests may need adaptation - document changes

### Test Failures

C test crashes or fails unexpectedly.

**Solutions:**
- Check if test requires kernel infrastructure
- Verify stub headers match kernel expectations
- Some tests are integration tests - may not work standalone
- Document adaptations needed

---

## Integration with Main Build

### CI Pipeline (Future)

```yaml
# .github/workflows/verify.yml
- name: Initialize submodules
  run: git submodule update --init --recursive

- name: Verify against Linux tests
  run: |
    cd tests/linux-tests
    just build-all
    just verify-list
    just verify-rbtree
```

### Pre-commit Hooks (Future)

```yaml
# lefthook.yml
pre-commit:
  commands:
    verify-tests:
      run: |
        cd tests/linux-tests
        just verify-list || echo "Warning: List verification failed"
```

---

## Files in This Directory

| File | Purpose |
|------|---------|
| `justfile` | Build commands for C tests |
| `README.md` | This file |
| `linux_types.h` | Stub headers (generated by `just`) |
| `test-*` | Compiled test executables |
| `*_c_output.txt` | Captured C test outputs |
| `*_rust_output.txt` | Captured Rust test outputs |

**Note:** Generated files are in `.gitignore` and rebuilt as needed.

---

## Design Principles

### 1. Test Traceability is King

Every test must be traceable to its Linux source:
- Document source file path
- Document function name
- Document line number (approximate)
- See **LINUX_TEST_MAPPING.md**

### 2. Use C Data Structures for I/O

Tests must operate on C-compatible structures:
- Input: `#[repr(C)]` structs
- Output: Verify C struct state
- Purpose: Binary-level compatibility verification

### 3. Deterministic Verification

No manual inspection, no AI comparison:
- Compile C code deterministically
- Compare automatically in tests
- CI fails if Rust diverges from C

### 4. Git Submodule for Versioning

Linux kernel as git submodule:
- Pins specific kernel version
- Easy to update
- Full source available for reference
- Proper attribution and licensing

---

## References

- **Linux kernel submodule**: `tests/linux-kernel/` (git submodule)
- **LINUX_KERNEL_LESSONS.md**: Design decisions #8 (just) and #9 (C reference)
- **LINUX_TEST_MAPPING.md**: Tracks all translated tests
- **.claude.md**: Core principles #11 (test translation) and #12 (traceability)
- **just documentation**: https://github.com/casey/just

---

## License

All Linux kernel code is GPL-2.0, matching Angzarr's license.

**SPDX-License-Identifier**: GPL-2.0

---

## Next Steps

1. ✅ Linux kernel submodule initialized
2. ✅ Build infrastructure with `just`
3. ✅ Documentation complete
4. ⏳ Create `build.rs` for Rust crates to compile C references
5. ⏳ Translate tests with C integration
6. ⏳ Verify all tests pass with C references

---

Last updated: 2025-11-16
