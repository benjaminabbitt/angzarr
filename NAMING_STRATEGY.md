# Naming Strategy and Dual Interface Approach

## Core Principle

**"Expose two interfaces: Linux-compatible and Angzarr native. Keep them clearly separated and consistently named."**

---

## Architecture Overview

```
┌─────────────────────────────────────────┐
│   C Code / Linux Modules                │
│   Uses: list_head, rb_node, pid_t       │ ← Linux-compatible interface
└──────────────┬──────────────────────────┘
               │
               │ #[no_mangle] extern "C"
               │
┌──────────────▼──────────────────────────┐
│  angzarr-linux-compat                   │
│  • snake_case (Linux style)             │
│  • C-compatible types                   │ ← Adapter Layer
│  • #[repr(C)] structs                   │
│  • Unsafe extern "C" functions          │
└──────────────┬──────────────────────────┘
               │
               │ Safe Rust API
               │
┌──────────────▼──────────────────────────┐
│  Angzarr Core Crates                    │
│  • PascalCase (Rust style)              │
│  • Safe abstractions                    │ ← Angzarr native interface
│  • Result<T, E> error handling          │
│  • Type-safe wrappers                   │
└─────────────────────────────────────────┘
```

---

## Naming Conventions

### Linux-Compatible Interface (angzarr-linux-compat)

**Location:** `angzarr-linux-compat/src/*.rs`

**Rules:**
1. Use exact Linux kernel names for structs and functions
2. snake_case for all identifiers (Linux C style)
3. Prefix with module name matches Linux headers
4. #[repr(C)] for all exported structs
5. #[no_mangle] for all exported functions
6. extern "C" calling convention

**Examples:**

```rust
// angzarr-linux-compat/src/list.rs

/// struct list_head - Linux-compatible list head
#[repr(C)]
pub struct list_head {
    pub next: *mut list_head,
    pub prev: *mut list_head,
}

/// INIT_LIST_HEAD - Initialize a list head
#[no_mangle]
pub unsafe extern "C" fn INIT_LIST_HEAD(list: *mut list_head) { /* ... */ }

/// list_add - Add new entry after specified head
#[no_mangle]
pub unsafe extern "C" fn list_add(new: *mut list_head, head: *mut list_head) { /* ... */ }

/// list_del - Delete entry from list
#[no_mangle]
pub unsafe extern "C" fn list_del(entry: *mut list_head) { /* ... */ }
```

**Type Aliases:**
```rust
// angzarr-linux-compat/src/types.rs

pub type pid_t = Pid;      // Linux name → Angzarr type
pub type uid_t = Uid;
pub type gid_t = Gid;
```

**Module Organization:**
```
angzarr-linux-compat/
├── src/
│   ├── lib.rs           # Re-exports all Linux-compatible APIs
│   ├── list.rs          # struct list_head, list_add, etc.
│   ├── rbtree.rs        # struct rb_node, rb_root, etc.
│   ├── types.rs         # pid_t, uid_t, gid_t
│   ├── error.rs         # errno conversion
│   └── mm.rs            # kmalloc, kfree (future)
```

---

### Angzarr Native Interface (Core Crates)

**Location:** `angzarr-*/src/*.rs` (except angzarr-linux-compat)

**Rules:**
1. Use Rust naming conventions (PascalCase for types, snake_case for functions)
2. Type-safe wrappers with newtype pattern
3. Safe abstractions with lifetimes where applicable
4. Result<T, KernelError> for fallible operations
5. No #[repr(C)] unless specifically needed
6. Private unsafe, public safe

**Examples:**

```rust
// angzarr-core/src/types.rs

/// Process ID with type safety
#[repr(transparent)]
pub struct Pid(pub i32);

impl Pid {
    pub fn new(id: i32) -> Self {
        Pid(id)
    }

    pub fn as_raw(&self) -> i32 {
        self.0
    }
}

/// User ID with type safety
#[repr(transparent)]
pub struct Uid(pub u32);

/// Reference counter with overflow protection
pub struct Kref {
    refcount: AtomicU32,
}

impl Kref {
    pub const fn new() -> Self { /* ... */ }
    pub fn get(&self) { /* ... */ }
    pub fn put(&self) -> bool { /* ... */ }
}
```

```rust
// angzarr-list/src/lib.rs

/// Intrusive list head (internal, matches Linux layout)
#[repr(C)]
pub struct ListHead {
    pub next: *mut ListHead,
    pub prev: *mut ListHead,
}

impl ListHead {
    /// Create new empty list
    pub const fn new() -> Self {
        ListHead {
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
        }
    }

    /// Initialize list to point to itself
    pub unsafe fn init(&mut self) { /* ... */ }

    /// Add entry after this one
    pub unsafe fn add(&mut self, new: *mut ListHead) { /* ... */ }
}

// Future: Safe owned list
pub struct List<T> {
    head: Option<Box<Node<T>>>,
}

impl<T> List<T> {
    pub fn new() -> Self { /* ... */ }
    pub fn push_front(&mut self, value: T) { /* ... */ }
    pub fn pop_front(&mut self) -> Option<T> { /* ... */ }
}
```

**Module Organization:**
```
angzarr-core/
├── src/
│   ├── lib.rs           # Core kernel types
│   ├── types.rs         # Pid, Uid, Gid, Kref
│   └── error.rs         # KernelError, KernelResult

angzarr-list/
├── src/
│   ├── lib.rs           # ListHead (intrusive), List<T> (owned, future)

angzarr-rbtree/
├── src/
│   ├── lib.rs           # RbNode, RbRoot, RbColor

angzarr-sync/
├── src/
│   ├── spinlock.rs      # Spinlock, SpinlockGuard
│   └── mutex.rs         # (future)
```

---

## Translation Pattern

The adapter layer translates between the two interfaces:

```rust
// Linux-compatible function (adapter layer)
#[no_mangle]
pub unsafe extern "C" fn list_add(new: *mut list_head, head: *mut list_head) {
    // Null checks (robustness)
    if new.is_null() || head.is_null() {
        return;
    }

    // Convert to Angzarr types
    let new_rust = new as *mut ListHead;
    let head_rust = &mut *(head as *mut ListHead);

    // Call Angzarr native function
    head_rust.add(new_rust);
}

// Angzarr native function (core)
impl ListHead {
    pub unsafe fn add(&mut self, new: *mut ListHead) {
        // Safe Rust implementation
        __list_add_raw(new, self as *mut _, self.next);
    }
}
```

---

## Interface Selection Guide

### Use Linux-Compatible Interface When:

✅ Writing C code that interfaces with Angzarr
✅ Porting existing Linux modules
✅ Maintaining binary compatibility with Linux
✅ External APIs that must match Linux exactly

**Example:**
```c
// C code using Linux-compatible interface
#include <linux/list.h>

struct my_data {
    int value;
    struct list_head list;
};

void example(void) {
    struct list_head head;
    INIT_LIST_HEAD(&head);

    struct my_data *item = kmalloc(sizeof(*item), GFP_KERNEL);
    list_add(&item->list, &head);
}
```

### Use Angzarr Native Interface When:

✅ Writing new Rust kernel code
✅ Internal Angzarr subsystem implementation
✅ Want type safety and lifetime guarantees
✅ Don't need Linux ABI compatibility

**Example:**
```rust
// Rust code using Angzarr native interface
use angzarr_core::{Pid, Uid};
use angzarr_list::List;  // Future safe owned list

fn example() {
    let pid = Pid::new(1000);
    let uid = Uid::new(1000);

    // Type safety: can't mix up Pid and Uid
    // set_uid(pid);  // ❌ Compile error

    let mut list = List::new();
    list.push_front(42);
    assert_eq!(list.pop_front(), Some(42));
}
```

---

## Crate Naming Convention

### Pattern: `angzarr-<subsystem>`

**Adapter Layer:**
- `angzarr-linux-compat` - Linux ABI compatibility adapter

**Core Subsystems:**
- `angzarr-core` - Core kernel types
- `angzarr-list` - List data structures
- `angzarr-rbtree` - Red-black tree
- `angzarr-sync` - Synchronization primitives
- `angzarr-mm` - Memory management
- `angzarr-sched` - Scheduler (future)
- `angzarr-fs` - Filesystem layer (future)
- `angzarr-net` - Network stack (future)
- `angzarr-drivers` - Device drivers (future)

**Infrastructure:**
- `angzarr-ffi` - FFI types and constants
- `angzarr-test-framework` - Testing utilities
- `angzarr-abi-test` - ABI compatibility tests
- `angzarr-kernel` - Bootable kernel binary

**Future:**
- `angzarr-event` - Event system / kernel bus
- `angzarr-security` - Security modules
- `angzarr-crypto` - Cryptographic primitives

---

## File Naming Convention

### Linux-Compatible Files (angzarr-linux-compat)

Match Linux header names:
- `list.rs` → corresponds to `<linux/list.h>`
- `rbtree.rs` → corresponds to `<linux/rbtree.h>`
- `types.rs` → corresponds to `<linux/types.h>`
- `sched.rs` → will correspond to `<linux/sched.h>` (future)
- `mm.rs` → will correspond to `<linux/mm.h>` (future)

### Angzarr Native Files

Use Rust module naming:
- `lib.rs` - Crate root
- `types.rs` - Type definitions
- `error.rs` - Error types
- `<feature>.rs` - Feature-specific modules

---

## Type Name Mapping

| Linux Type | Linux-Compat | Angzarr Native | Notes |
|------------|--------------|----------------|-------|
| `struct list_head` | `list_head` | `ListHead` | Binary compatible |
| `struct rb_node` | `rb_node` | `RbNode` | Binary compatible |
| `struct rb_root` | `rb_root` | `RbRoot` | Binary compatible |
| `pid_t` | `pid_t` | `Pid` | Type alias in compat |
| `uid_t` | `uid_t` | `Uid` | Type alias in compat |
| `gid_t` | `gid_t` | `Gid` | Type alias in compat |
| `struct kref` | `kref` | `Kref` | Reference counter |
| `spinlock_t` | `spinlock_t` | `Spinlock` | Lock type |
| `gfp_t` | `gfp_t` | `GfpFlags` | Allocation flags |

---

## Function Name Mapping

| Linux Function | Linux-Compat | Angzarr Native | Notes |
|----------------|--------------|----------------|-------|
| `INIT_LIST_HEAD()` | `INIT_LIST_HEAD()` | `ListHead::init()` | Macro vs method |
| `list_add()` | `list_add()` | `ListHead::add()` | Function vs method |
| `list_del()` | `list_del()` | `ListHead::del()` | Function vs method |
| `rb_insert()` | `rb_insert()` | `RbRoot::insert()` | Function vs method |
| `kmalloc()` | `kmalloc()` | `allocate()` | Different names |
| `kfree()` | `kfree()` | `deallocate()` | Different names |
| `spin_lock()` | `spin_lock()` | `Spinlock::lock()` | Function vs method |

---

## Error Handling Patterns

### Linux-Compatible (errno)

```rust
// angzarr-linux-compat/src/mm.rs

/// kmalloc - Allocate memory (Linux-compatible)
#[no_mangle]
pub unsafe extern "C" fn kmalloc(size: usize, flags: gfp_t) -> *mut u8 {
    match angzarr_mm::allocate(size, flags.into()) {
        Ok(ptr) => ptr,
        Err(_) => core::ptr::null_mut(),  // NULL on error
    }
}

/// errno-based error function
#[no_mangle]
pub extern "C" fn do_operation() -> c_int {
    match internal_operation() {
        Ok(_) => 0,
        Err(e) => e.to_errno(),  // Negative errno
    }
}
```

### Angzarr Native (Result)

```rust
// angzarr-mm/src/allocator.rs

/// Allocate memory (Angzarr native)
pub fn allocate(size: usize, flags: GfpFlags) -> KernelResult<*mut u8> {
    if size == 0 {
        return Err(KernelError::EINVAL);
    }

    if size > MAX_ALLOCATION {
        return Err(KernelError::ENOMEM);
    }

    // Actual allocation
    Ok(ptr)
}
```

---

## Documentation Conventions

### Linux-Compatible

```rust
/// list_add - Add a new entry
///
/// Linux equivalent: `list_add(new, head)`
///
/// Insert a new entry after the specified head.
/// This is good for implementing stacks.
///
/// # Safety
///
/// - Both `new` and `head` must be valid, non-null pointers
/// - `new` must not already be in a list
#[no_mangle]
pub unsafe extern "C" fn list_add(new: *mut list_head, head: *mut list_head) {
    // ...
}
```

### Angzarr Native

```rust
/// Add a new entry after this list head
///
/// # Safety
///
/// - `new` must be a valid pointer to an initialized ListHead
/// - `new` must not already be in a list
/// - The list must be properly initialized via `init()`
///
/// # Examples
///
/// ```
/// let mut head = ListHead::new();
/// unsafe {
///     head.init();
///     head.add(new_entry);
/// }
/// ```
pub unsafe fn add(&mut self, new: *mut ListHead) {
    // ...
}
```

---

## Testing Approach

### Linux-Compatible Interface Tests

```rust
// angzarr-linux-compat/src/list.rs

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr;

    #[test]
    fn test_list_add() {
        let mut head = list_head {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        };
        let mut entry = list_head {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        };

        unsafe {
            INIT_LIST_HEAD(&mut head);
            list_add(&mut entry, &mut head);

            assert!(!list_empty(&head));
            assert_eq!(head.next, &mut entry as *mut _);
        }
    }
}
```

### Angzarr Native Interface Tests

```rust
// angzarr-list/src/lib.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_head_init() {
        let mut head = ListHead::new();
        unsafe {
            head.init();
            let head_ptr = &mut head as *mut _;
            assert_eq!(head.next, head_ptr);
            assert_eq!(head.prev, head_ptr);
        }
    }
}
```

---

## Summary

### Key Principles

1. **Two Distinct Interfaces**
   - Linux-compatible: C naming, unsafe, #[repr(C)]
   - Angzarr native: Rust naming, safe, typed

2. **Clear Separation**
   - Adapter layer in `angzarr-linux-compat`
   - Core implementation in other `angzarr-*` crates

3. **Consistent Naming**
   - Linux interface: snake_case, exact Linux names
   - Angzarr interface: PascalCase/snake_case per Rust conventions

4. **Type Safety**
   - Linux interface: type aliases to Angzarr types
   - Angzarr interface: newtype wrappers, compile-time checks

5. **Documentation**
   - Always document Linux equivalent in adapter
   - Document safety requirements for unsafe functions
   - Link between interfaces in documentation

### Benefits

✅ **Compatibility**: Perfect Linux ABI match
✅ **Safety**: Rust guarantees in internal code
✅ **Clarity**: Obvious which interface you're using
✅ **Evolution**: Can improve internals without breaking ABI
✅ **Testing**: Both interfaces tested independently

---

## References

- `ADAPTER_LAYER.md` - Adapter architecture details
- `LINUX_KERNEL_LESSONS.md` - Design decisions and rationale
- `.claude.md` - Development principles
- `angzarr-linux-compat/README.md` - Adapter usage guide
