# Angzarr ABI Compatibility Testing

This crate provides comprehensive ABI (Application Binary Interface) compatibility testing to ensure Rust structures are binary-compatible with Linux kernel C structures.

## Purpose

Angzarr maintains 100% binary compatibility with the Linux kernel. This test suite verifies:

1. **Structure Sizes**: Rust structs match Linux C struct sizes exactly
2. **Alignment**: Memory alignment matches Linux kernel requirements
3. **Field Offsets**: Structure field positions match byte-for-byte
4. **Constant Values**: GFP flags, error codes, etc. match Linux values
5. **ABI Stability**: C function call interfaces remain compatible

## Test Categories

### 1. Structure Layout Tests

Located in `tests/*_compat.rs`:

- **list_compat.rs**: Verifies `ListHead` matches `struct list_head`
- **rbtree_compat.rs**: Verifies `RbNode`/`RbRoot` match Linux rb_tree
- **types_compat.rs**: Verifies `Pid`, `Uid`, `Kref` match kernel types
- **ffi_compat.rs**: Verifies GFP flags and error codes

### 2. C Reference Tests

Located in `tests/c_reference_compat.rs`:

Compiles actual C structure definitions and compares against Rust at runtime.

Uses:
- `build.rs` to compile C reference code
- FFI to call C functions that return size/offset info
- Direct comparison of compiled structure layouts

### 3. Compile-Time Assertions

Uses `static_assertions` crate for compile-time verification:

```rust
assert_eq_size!(ListHead, [usize; 2]);
assert_eq_align!(ListHead, usize);
```

## Running Tests

```bash
# Run all ABI compatibility tests
cargo test -p angzarr-abi-test

# Run specific test category
cargo test -p angzarr-abi-test list_compat
cargo test -p angzarr-abi-test rbtree_compat

# Run C reference comparison tests
cargo test -p angzarr-abi-test c_reference

# Verbose output
cargo test -p angzarr-abi-test -- --nocapture
```

## Test Methodology

### Size Verification

```rust
#[test]
fn test_list_head_size() {
    assert_eq!(
        core::mem::size_of::<ListHead>(),
        2 * core::mem::size_of::<usize>(),
        "ListHead must be exactly 16 bytes on 64-bit"
    );
}
```

### Offset Verification

```rust
use memoffset::offset_of;

#[test]
fn test_list_head_offsets() {
    assert_eq!(offset_of!(ListHead, next), 0);
    assert_eq!(offset_of!(ListHead, prev), 8); // On 64-bit
}
```

### C Comparison

```c
// In build.rs compiled C code
size_t list_head_size(void) {
    return sizeof(struct list_head);
}
```

```rust
extern "C" {
    fn list_head_size() -> usize;
}

#[test]
fn test_vs_c() {
    unsafe {
        assert_eq!(
            core::mem::size_of::<ListHead>(),
            list_head_size()
        );
    }
}
```

## Platform-Specific Tests

Tests handle different pointer widths:

```rust
#[cfg(target_pointer_width = "64")]
#[test]
fn test_list_head_size_64bit() {
    assert_eq!(core::mem::size_of::<ListHead>(), 16);
}

#[cfg(target_pointer_width = "32")]
#[test]
fn test_list_head_size_32bit() {
    assert_eq!(core::mem::size_of::<ListHead>(), 8);
}
```

## Verification Macros

The crate provides helper macros for ABI verification:

```rust
// Verify size
verify_size!(ListHead, 16);

// Verify field offset
verify_offset!(ListHead, next, 0);

// Verify alignment
verify_align!(ListHead, 8);
```

## Adding New Structure Tests

To add tests for a new structure:

1. Create `tests/{module}_compat.rs`
2. Add size, alignment, and offset tests
3. Add compile-time assertions
4. Update `build.rs` with C reference code
5. Add C comparison tests

Example template:

```rust
use your_module::YourStruct;
use memoffset::offset_of;
use static_assertions::*;

#[test]
fn test_your_struct_size() {
    assert_eq!(
        core::mem::size_of::<YourStruct>(),
        EXPECTED_SIZE
    );
}

#[test]
fn test_your_struct_alignment() {
    assert_eq!(
        core::mem::align_of::<YourStruct>(),
        EXPECTED_ALIGN
    );
}

#[test]
fn test_your_struct_offsets() {
    assert_eq!(offset_of!(YourStruct, field1), OFFSET1);
    assert_eq!(offset_of!(YourStruct, field2), OFFSET2);
}

assert_eq_size!(YourStruct, ExpectedType);
```

## CI Integration

These tests run automatically in CI:

```bash
# In CI pipeline
just test  # Includes angzarr-abi-test
```

All tests must pass before code can be merged.

## Debugging ABI Issues

If tests fail:

1. **Check repr(C)**: Ensure struct has `#[repr(C)]`
2. **Verify field types**: Match Linux kernel types exactly
3. **Check alignment**: Use proper alignment attributes
4. **Compare with Linux**: Check `include/linux/*.h` headers
5. **Use pahole**: Analyze structure layout with `pahole` tool

```bash
# Analyze Rust structure
cargo rustc -- --emit=obj
pahole target/debug/libangzarr_list.so

# Compare with C
pahole /path/to/vmlinux
```

## References

- Linux Kernel Headers: `/usr/src/linux/include/`
- memoffset crate: https://docs.rs/memoffset
- static_assertions: https://docs.rs/static_assertions
- bindgen: https://rust-lang.github.io/rust-bindgen/

## Success Criteria

All tests must:
- ✅ Pass on x86_64 (64-bit)
- ✅ Pass on x86 (32-bit)
- ✅ Pass on ARM64 (if applicable)
- ✅ Match Linux kernel 5.15+ layout
- ✅ Maintain backward compatibility

## Maintenance

This test suite must be updated when:
- Adding new kernel data structures
- Updating Linux kernel version
- Changing existing structure definitions
- Adding platform support

**Remember**: Binary compatibility is NON-NEGOTIABLE. All ABI tests must pass.
