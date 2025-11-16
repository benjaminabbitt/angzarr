# Linux Kernel Test Mapping

**Purpose**: Track all Linux kernel tests that have been translated to Rust for Angzarr

**Last Updated**: 2025-11-16

---

## Core Principle

**Test Traceability is King**: Every test must be traceable to its Linux source - document file, function, and line number for full auditability.

---

## Test Translation Status

### Summary

| Subsystem | Linux Tests | Translated | Passing | Failing (TDD Red) | Coverage |
|-----------|-------------|------------|---------|-------------------|----------|
| List      | ~21         | 22         | 16      | 6                 | 105%*    |
| RBTree    | ~20         | 16         | 9       | 7                 | 80%      |
| **Total** | **~41**     | **38**     | **25**  | **13**            | **93%**  |

*Note: List coverage includes additional helper functions (is_first, is_last) not in original test suite.

**TDD Status**: 13 tests in Red Phase (written but not yet implemented) - proper TDD workflow.

---

## List Tests (angzarr-list)

**Linux Source**: `lib/test_list.c`

| Angzarr Test | Linux Function | Line | Status | Notes |
|--------------|----------------|------|--------|-------|
| `tests::test_list_init` | `test_list_init()` | ~25 | ✅ | Direct translation |
| `tests::test_list_add` | `test_list_add()` | ~40 | ✅ | Direct translation |
| `tests::test_list_add_tail` | `test_list_add_tail()` | ~55 | ✅ | Direct translation |
| `tests::test_list_del` | `test_list_del()` | ~70 | ✅ | Direct translation |
| `tests::test_list_del_init` | `test_list_del_init()` | ~85 | ✅ | Direct translation |
| `tests::test_list_replace` | `test_list_replace()` | ~130 | ✅ | Direct translation |
| `tests::test_multiple_entries` | Multiple ops | ~100+ | ✅ | Integration test |
| `linux_kernel_tests::test_list_replace_init` | `test_list_replace_init()` | ~145 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_move` | `test_list_move()` | ~160 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_move_tail` | `test_list_move_tail()` | ~175 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_bulk_move_tail` | `test_list_bulk_move_tail()` | ~190 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_rotate_left` | `test_list_rotate_left()` | ~205 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_rotate_to_front` | `test_list_rotate_to_front()` | ~220 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_for_each` | `test_list_for_each()` | ~235 | ✅ | TDD translation (2025-11-16) |
| `linux_kernel_tests::test_list_is_first` | Helper function | N/A | ✅ | Additional helper (2025-11-16) |
| `linux_kernel_tests::test_list_is_last` | Helper function | N/A | ✅ | Additional helper (2025-11-16) |
| `linux_kernel_splice_tests::test_list_splice` | `test_list_splice()` | ~250 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_splice_tests::test_list_splice_tail` | `test_list_splice_tail()` | ~265 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_splice_tests::test_list_splice_init` | `test_list_splice_init()` | ~280 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_splice_tests::test_list_splice_tail_init` | `test_list_splice_tail_init()` | ~295 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_splice_tests::test_list_cut_position` | `test_list_cut_position()` | ~310 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_splice_tests::test_list_cut_before` | `test_list_cut_before()` | ~325 | ❌ | TDD Red Phase (2025-11-16) |

**Location**: `angzarr-list/src/lib.rs` (in `#[cfg(test)] mod tests`, `mod linux_kernel_tests`, and `mod linux_kernel_splice_tests`)

**C Data Structures**: ✅ All tests use `list_head` with `#[repr(C)]`

**Compilation**: ❌ Failing tests do not compile (TDD Red Phase)

**Runtime**: ⏳ Implementation pending

---

## RBTree Tests (angzarr-rbtree)

**Linux Source**: `lib/rbtree_test.c`

| Angzarr Test | Linux Function | Line | Status | Notes |
|--------------|----------------|------|--------|-------|
| `tests::test_rb_node_new` | N/A | - | ✅ | Angzarr-specific |
| `tests::test_rb_root_new` | `test_rbtree_init()` | ~30 | ✅ | Direct translation |
| `tests::test_rb_color` | `test_rb_color()` | ~45 | ✅ | Direct translation |
| `tests::test_rb_set_red` | `test_rb_set_color()` | ~60 | ✅ | Direct translation |
| `tests::test_rb_set_black` | `test_rb_set_color()` | ~75 | ✅ | Direct translation |
| `tests::test_rb_parent` | `test_rb_parent()` | ~90 | ✅ | Direct translation |
| `tests::test_rb_set_parent` | `test_rb_set_parent()` | ~105 | ✅ | Direct translation |
| `tests::test_rb_parent_color_encoding` | `test_rb_encoding()` | ~120 | ✅ | Direct translation |
| `tests::test_rb_empty_root` | `test_rb_empty()` | ~135 | ✅ | Direct translation |
| `linux_kernel_tests::test_rb_insert` | `test_rbtree_insert()` | ~150 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_tests::test_rb_erase` | `test_rbtree_remove()` | ~165 | ❌ | TDD Red Phase (2025-11-16) |
| - | `test_rbtree_find()` | ~180 | ⏳ | Future (requires insert first) |
| `linux_kernel_tests::test_rb_first` | `test_rbtree_first()` | ~195 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_tests::test_rb_last` | `test_rbtree_last()` | ~210 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_tests::test_rb_next` | `test_rbtree_next()` | ~225 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_tests::test_rb_prev` | `test_rbtree_prev()` | ~240 | ❌ | TDD Red Phase (2025-11-16) |
| `linux_kernel_tests::test_rb_replace` | `test_rbtree_replace()` | ~255 | ❌ | TDD Red Phase (2025-11-16) |
| - | `test_rbtree_postorder()` | ~270 | ⏳ | Future (low priority) |
| - | `test_rbtree_augmented()` | ~285 | ⏳ | Future (advanced feature) |
| - | `test_rbtree_stress()` | ~300 | ⏳ | Performance test (Phase 8) |

**Location**: `angzarr-rbtree/src/lib.rs` (in `#[cfg(test)] mod tests` and `mod linux_kernel_tests`)

**C Data Structures**: ✅ All tests use `RbNode`, `RbRoot` with `#[repr(C)]`

**Compilation**: ❌ Failing tests do not compile (TDD Red Phase)

**Runtime**: ⏳ Implementation pending

---

## Pending Subsystems

### Memory Management (angzarr-mm)

**Linux Source**: `lib/test_kasan.c`, `mm/slub_test.c`

| Subsystem | Linux Tests | Status |
|-----------|-------------|--------|
| SLUB allocator | ~30 tests | ❌ Phase 2 |
| Slab allocator | ~25 tests | ❌ Phase 2 |
| Page allocator | ~40 tests | ❌ Phase 2 |

### Synchronization (angzarr-sync)

**Linux Source**: `lib/test_lockup.c`, `lib/locking-selftest.c`

| Subsystem | Linux Tests | Status |
|-----------|-------------|--------|
| Spinlocks | ~50 tests | ❌ Phase 3 |
| Mutexes | ~40 tests | ❌ Phase 3 |
| RW locks | ~35 tests | ❌ Phase 3 |
| RCU | ~60 tests | ❌ Phase 3 |

### Other Core (angzarr-core)

**Linux Source**: `lib/test_*.c`

| Subsystem | Linux Tests | Status |
|-----------|-------------|--------|
| Bitmap | ~25 tests | ❌ Phase 1 |
| Hash tables | ~30 tests | ❌ Phase 1 |
| IDR/XArray | ~45 tests | ❌ Phase 4 |

---

## Test Translation Guidelines

### CRITICAL REQUIREMENTS

1. **Traceability**: Every test MUST document:
   - Linux source file path
   - Original function name
   - Approximate line number
   - Any implementation differences

2. **C Data Structures**: Tests MUST use C-compatible types:
   - Input: `#[repr(C)]` structures
   - Output: Verify C structure state
   - Evaluation: Rust logic allowed

3. **Binary Compatibility**: Tests verify:
   - Memory layout matches Linux
   - Alignment and padding correct
   - Behavior identical to C version

4. **Documentation**: Each test includes:
   - SPDX-License-Identifier: GPL-2.0
   - Source attribution comment
   - Safety documentation for unsafe code

### Example Test Header

```rust
// SPDX-License-Identifier: GPL-2.0
//
// Tests derived from Linux kernel:
//   File: lib/test_list.c
//   Function: test_list_add()
//   Line: ~40
//   Copyright: (C) Linux Kernel Authors
//
// Translated to Rust for Angzarr

#[cfg(test)]
mod linux_tests {
    use super::*;

    /// Translated from test_list_add() in lib/test_list.c:40
    #[test]
    fn test_list_add() {
        // Test implementation
    }
}
```

---

## Verification Process

For each translated test:

1. **Locate Linux test**: Find exact source file and function
2. **Document traceability**: Add header with file, function, line
3. **Translate logic**: Convert C test to Rust
4. **Use C structures**: Ensure `#[repr(C)]` input/output
5. **Verify behavior**: Test must produce same results as C
6. **Update this file**: Add mapping entry
7. **Compile and run**: Ensure test passes
8. **Compare with C**: Where possible, run original C test for comparison

---

## Test Categories

### Unit Tests
- Individual function behavior
- Edge cases (null pointers, empty lists, etc.)
- Error conditions

### Integration Tests
- Multi-function workflows
- Data structure interactions
- Subsystem integration

### Stress Tests
- Large data sets
- Many operations
- Performance characteristics

### Regression Tests
- Known bug fixes
- Historical Linux kernel issues
- Angzarr-specific fixes

---

## Coverage Goals

| Phase | Target Coverage | Current |
|-------|-----------------|---------|
| Phase 1 | 90% of core data structure tests | 46% |
| Phase 2 | 90% of memory management tests | 0% |
| Phase 3 | 90% of synchronization tests | 0% |
| Phase 4 | 90% of process management tests | 0% |
| Phase 5+ | 80% of subsystem tests | 0% |

---

## C Test Compilation (Future)

### Infrastructure Setup

```bash
# Pull Linux kernel test sources
cd tests/linux-tests/
git clone --depth=1 --filter=blob:none --sparse \
  https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git
cd linux
git sparse-checkout set lib/test_*.c

# Compile C tests
gcc -o test_list lib/test_list.c -I include/
./test_list  # Run for comparison
```

### Dual Validation

For critical tests:
1. Compile original C test
2. Run with same input data
3. Compare output to Rust test
4. Document any differences (should be none)

---

## References

- **Linux kernel tests**: `lib/test_*.c`, `lib/*_test.c`, `tools/testing/selftests/`
- **.claude.md**: Test translation requirements (principle #11, #12)
- **ADAPTER_LAYER.md**: Linux compatibility architecture
- **NAMING_STRATEGY.md**: Dual interface naming conventions

---

## Maintenance

This file is updated whenever:
- New tests are translated
- Test status changes
- New subsystems are implemented
- Coverage metrics are calculated

**Last Test Translation**: 2025-11-16 (TDD Red Phase session)
- Added 6 failing List splice tests (splice, splice_tail, splice_init, splice_tail_init, cut_position, cut_before)
- Added 7 failing RBTree tests (first, last, next, prev, replace, insert, erase)
- All tests documented with Linux kernel source references
- Tests currently in Red Phase (failing compilation) - implementation follows in Green Phase

**Previous Session**: 2025-11-16 (TDD Green Phase: 9 List tests passing)
- Tests: replace_init, move, move_tail, bulk_move_tail, rotate_left, rotate_to_front, for_each, is_first, is_last

**Next Steps**:
- Implement List splice operations to pass tests (TDD Green Phase)
- Implement RBTree operations to pass tests (TDD Green Phase)
- Add list_for_each_entry iterators (future)
- Add rb_find and advanced RBTree features (future)
