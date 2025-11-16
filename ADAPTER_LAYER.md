# Angzarr Adapter Layer Architecture

## Overview

Angzarr uses a **boundary/adapter layer** architecture to maintain Linux compatibility without constraining internal design decisions. This allows Angzarr to make optimal design choices internally while providing a perfect Linux-compatible external interface.

## Design Philosophy

### Core Principle

**"Linux compatibility is a translation layer, not a constraint."**

- **Angzarr Core**: Clean, safe, idiomatic Rust code with optimal design
- **Adapter Layer**: Translates between Angzarr and Linux interfaces
- **Linux Boundary**: Perfect C ABI compatibility for external consumers

### Advantages

1. **Freedom to Innovate**: Internal code can use best Rust practices
2. **Safety First**: Core uses safe abstractions, adapters handle unsafe
3. **Maintainability**: Clear separation of concerns
4. **Testability**: Test Rust API separately from C compatibility
5. **Evolution**: Can improve internals without breaking Linux ABI

## Architecture Layers

```
┌─────────────────────────────────────────────────────────┐
│  Linux Kernel Modules / User Code (C)                  │
│  Expects: Linux C ABI                                   │
└────────────────┬────────────────────────────────────────┘
                 │
                 │ C ABI Interface
                 │ #[no_mangle] extern "C"
                 │
┌────────────────▼────────────────────────────────────────┐
│  ADAPTER LAYER (angzarr-linux-compat)                   │
│                                                          │
│  • Translates C calls to Rust                           │
│  • Maintains #[repr(C)] structs                         │
│  • Provides Linux-compatible error codes                │
│  • Handles pointer conversions                          │
│  • Zero-cost abstractions                               │
└────────────────┬────────────────────────────────────────┘
                 │
                 │ Safe Rust API
                 │ Type-safe, ownership-aware
                 │
┌────────────────▼────────────────────────────────────────┐
│  ANGZARR CORE (angzarr-*)                               │
│                                                          │
│  • Idiomatic Rust code                                  │
│  • Safe abstractions                                    │
│  • Optimal data structures                              │
│  • Rust error handling (Result)                         │
│  • No Linux constraints                                 │
└─────────────────────────────────────────────────────────┘
```

## Example: Linked List

### Internal Angzarr Design (Optimal Rust)

```rust
// angzarr-list/src/lib.rs
// Pure Rust, safe, idiomatic

/// A safe, type-safe intrusive linked list
pub struct List<T> {
    head: Option<NonNull<Node<T>>>,
    tail: Option<NonNull<Node<T>>>,
    len: usize,
}

pub struct Node<T> {
    next: Option<NonNull<Node<T>>>,
    prev: Option<NonNull<Node<T>>>,
    data: T,
}

impl<T> List<T> {
    /// Create a new empty list
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    /// Add item to front of list (safe, takes ownership)
    pub fn push_front(&mut self, data: T) {
        // Safe Rust implementation
    }

    /// Remove item from front (returns Option<T>)
    pub fn pop_front(&mut self) -> Option<T> {
        // Safe Rust implementation
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
```

### Adapter Layer (Linux Compatible)

```rust
// angzarr-linux-compat/src/list.rs
// Translates between Linux C API and Angzarr Rust API

use angzarr_list::{List, Node};

/// Linux-compatible list_head (C ABI)
#[repr(C)]
pub struct list_head {
    pub next: *mut list_head,
    pub prev: *mut list_head,
}

/// Linux-compatible functions (extern "C")
#[no_mangle]
pub unsafe extern "C" fn INIT_LIST_HEAD(list: *mut list_head) {
    if list.is_null() {
        return;
    }
    (*list).next = list;
    (*list).prev = list;
}

#[no_mangle]
pub unsafe extern "C" fn list_add(
    new: *mut list_head,
    head: *mut list_head,
) {
    if new.is_null() || head.is_null() {
        return;
    }

    // Translate to Angzarr API
    // (Implementation uses Angzarr's safe abstractions internally)
    __list_add_internal(new, head, (*head).next);
}

#[no_mangle]
pub unsafe extern "C" fn list_empty(head: *const list_head) -> bool {
    if head.is_null() {
        return true;
    }
    (*head).next == head as *mut list_head
}

// Internal helper that can use Angzarr's safe API
unsafe fn __list_add_internal(
    new: *mut list_head,
    prev: *mut list_head,
    next: *mut list_head,
) {
    (*next).prev = new;
    (*new).next = next;
    (*new).prev = prev;
    (*prev).next = new;
}
```

## Adapter Pattern Details

### Layer Responsibilities

#### 1. Angzarr Core Layer

**Responsibilities:**
- Implement optimal data structures in Rust
- Use safe abstractions wherever possible
- Provide ergonomic Rust API
- Focus on correctness and performance
- No Linux-specific constraints

**Characteristics:**
- Generic over types: `List<T>`, `Tree<K, V>`
- Uses `Option`, `Result`, `NonNull`
- Ownership and borrowing
- Iterator support
- No raw pointers in public API

**Example:**
```rust
// Core can make optimal decisions
pub struct RBTree<K: Ord, V> {
    root: Option<Box<Node<K, V>>>,
    len: usize,
}

impl<K: Ord, V> RBTree<K, V> {
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // Safe Rust implementation
    }
}
```

#### 2. Adapter Layer

**Responsibilities:**
- Translate C calls to Rust calls
- Maintain `#[repr(C)]` structures
- Handle raw pointers safely
- Convert errors to errno
- Zero-cost translation
- Perfect ABI compatibility

**Characteristics:**
- `#[repr(C)]` structs
- `#[no_mangle]` functions
- `extern "C"` calling convention
- Unsafe boundary well-documented
- Defensive null checks

**Example:**
```rust
// Adapter provides Linux compatibility
#[repr(C)]
pub struct rb_root {
    rb_node: *mut rb_node,
}

#[no_mangle]
pub unsafe extern "C" fn rb_insert(
    root: *mut rb_root,
    node: *mut rb_node,
) -> i32 {
    if root.is_null() || node.is_null() {
        return -EINVAL;
    }

    // Translate to Angzarr's safe API
    match angzarr_rbtree::insert_from_raw(root, node) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}
```

## Error Handling Translation

### Angzarr Core (Rust)

```rust
// angzarr-core/src/error.rs

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AngzarrError {
    OutOfMemory,
    InvalidArgument,
    PermissionDenied,
    NotFound,
    Busy,
}

pub type Result<T> = core::result::Result<T, AngzarrError>;
```

### Adapter Layer (Linux)

```rust
// angzarr-linux-compat/src/error.rs

impl AngzarrError {
    /// Convert to Linux errno
    pub fn to_errno(self) -> i32 {
        match self {
            AngzarrError::OutOfMemory => -ENOMEM,      // -12
            AngzarrError::InvalidArgument => -EINVAL,  // -22
            AngzarrError::PermissionDenied => -EPERM,  // -1
            AngzarrError::NotFound => -ENOENT,         // -2
            AngzarrError::Busy => -EBUSY,              // -16
        }
    }

    /// Convert from Linux errno
    pub fn from_errno(errno: i32) -> Option<Self> {
        match -errno {
            ENOMEM => Some(AngzarrError::OutOfMemory),
            EINVAL => Some(AngzarrError::InvalidArgument),
            EPERM => Some(AngzarrError::PermissionDenied),
            ENOENT => Some(AngzarrError::NotFound),
            EBUSY => Some(AngzarrError::Busy),
            _ => None,
        }
    }
}
```

## Memory Management Translation

### Angzarr Core (Safe Rust)

```rust
// angzarr-mm/src/allocator.rs

/// Safe memory allocator
pub struct Allocator {
    // Internal implementation
}

impl Allocator {
    /// Allocate memory (returns Result)
    pub fn allocate(&mut self, layout: Layout) -> Result<NonNull<u8>> {
        // Safe implementation
    }

    /// Deallocate memory (takes ownership)
    pub fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        // Safe implementation
    }
}

/// Safe allocation helper
pub fn alloc<T>() -> Result<Box<T>> {
    // Safe Rust allocation
}
```

### Adapter Layer (Linux kmalloc)

```rust
// angzarr-linux-compat/src/mm.rs

#[no_mangle]
pub unsafe extern "C" fn kmalloc(size: usize, flags: u32) -> *mut u8 {
    let layout = match Layout::from_size_align(size, 8) {
        Ok(l) => l,
        Err(_) => return core::ptr::null_mut(),
    };

    // Translate GFP flags to Angzarr allocation policy
    let policy = gfp_to_policy(flags);

    // Use Angzarr's safe allocator
    match GLOBAL_ALLOCATOR.allocate(layout, policy) {
        Ok(ptr) => ptr.as_ptr(),
        Err(_) => core::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn kfree(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let ptr = NonNull::new_unchecked(ptr);
    // Translate to Angzarr's safe deallocation
    GLOBAL_ALLOCATOR.deallocate(ptr);
}

// Helper: translate Linux GFP flags to Angzarr policy
fn gfp_to_policy(gfp: u32) -> AllocationPolicy {
    if gfp & GFP_ATOMIC != 0 {
        AllocationPolicy::Atomic
    } else if gfp & GFP_KERNEL != 0 {
        AllocationPolicy::Sleepable
    } else {
        AllocationPolicy::NoWait
    }
}
```

## Type Conversion Examples

### Generic to Concrete

**Angzarr Core:**
```rust
// Generic, reusable
pub struct List<T> { ... }
```

**Adapter:**
```rust
// Concrete for Linux ABI
#[repr(C)]
pub struct list_head {
    next: *mut list_head,
    prev: *mut list_head,
}

// list_head is effectively List<()> with C layout
```

### Safe to Unsafe

**Angzarr Core:**
```rust
pub fn find_entry(&self, key: &K) -> Option<&V> {
    // Safe traversal with references
}
```

**Adapter:**
```rust
#[no_mangle]
pub unsafe extern "C" fn find_entry_raw(
    tree: *const rb_root,
    key: *const c_void,
) -> *mut c_void {
    if tree.is_null() || key.is_null() {
        return core::ptr::null_mut();
    }

    // SAFETY: Caller guarantees pointers are valid
    let tree_ref = &*tree;
    let key_ref = &*(key as *const K);

    match angzarr_tree::find(tree_ref, key_ref) {
        Some(value) => value as *const V as *mut c_void,
        None => core::ptr::null_mut(),
    }
}
```

## Benefits of This Architecture

### 1. Internal Freedom

Angzarr can:
- Use Rust idioms (Option, Result, iterators)
- Make breaking changes internally without affecting Linux ABI
- Optimize without ABI constraints
- Add Rust-specific features (async, etc.)

### 2. Perfect Compatibility

Linux sees:
- Exact same ABI as C kernel
- Same structure layouts
- Same function signatures
- Same error codes
- Zero behavioral differences

### 3. Safety

- Unsafe code isolated to adapter layer
- Core code is safe Rust
- Clear boundary between safe/unsafe
- Easier to audit and verify

### 4. Testing

- Test Rust API with safe Rust tests
- Test C API with ABI compatibility tests
- Test adapter translation separately
- Mock Linux calls without kernel

### 5. Evolution

- Can replace adapter with different backends
- Could support other kernels (BSD, etc.)
- Internal improvements don't break compatibility
- Forward evolution path

## Crate Organization

```
angzarr/
├── angzarr-core/          # Pure Rust core (no Linux constraints)
├── angzarr-list/          # Rust list implementation
├── angzarr-rbtree/        # Rust rbtree implementation
├── angzarr-mm/            # Rust memory management
├── angzarr-sync/          # Rust synchronization
│
├── angzarr-linux-compat/  # ADAPTER LAYER (NEW)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── list.rs        # list_head → List<T> adapter
│   │   ├── rbtree.rs      # rb_tree → RBTree adapter
│   │   ├── mm.rs          # kmalloc → Allocator adapter
│   │   ├── sync.rs        # spinlock → Spinlock adapter
│   │   └── error.rs       # errno conversion
│   └── include/           # Generated C headers
│       └── linux/
│           ├── list.h     # Linux-compatible headers
│           ├── rbtree.h
│           └── types.h
│
└── angzarr-ffi/           # Low-level FFI types (kept for backward compat)
```

## Migration Strategy

### Phase 1: Create Adapter Layer

1. Create `angzarr-linux-compat` crate
2. Move C-compatible code from core crates to adapter
3. Keep core crates pure Rust

### Phase 2: Refactor Core

1. Make core APIs more Rust-idiomatic
2. Use safe abstractions
3. Remove Linux-specific constraints

### Phase 3: Optimize

1. Improve core implementations
2. Adapter remains stable
3. Linux ABI never breaks

## Best Practices

### For Core Development

✅ **DO:**
- Use safe Rust
- Make optimal design decisions
- Use generic types
- Provide ergonomic APIs
- Focus on correctness

❌ **DON'T:**
- Worry about C compatibility
- Use raw pointers in public API
- Constrain design for Linux
- Mix unsafe with safe code

### For Adapter Development

✅ **DO:**
- Document all unsafe code
- Check for null pointers
- Convert errors properly
- Match Linux behavior exactly
- Test ABI compatibility

❌ **DON'T:**
- Add business logic
- Make design decisions
- Change core APIs
- Leak unsafe outside adapter

## Example: Complete Flow

### 1. User Code (C)

```c
#include <linux/list.h>

struct my_data {
    int value;
    struct list_head list;
};

struct list_head my_list;
INIT_LIST_HEAD(&my_list);

struct my_data *item = kmalloc(sizeof(*item), GFP_KERNEL);
item->value = 42;
list_add(&item->list, &my_list);
```

### 2. Adapter Layer Translation

```rust
// angzarr-linux-compat/src/list.rs

#[no_mangle]
pub unsafe extern "C" fn list_add(new: *mut list_head, head: *mut list_head) {
    // Null checks
    if new.is_null() || head.is_null() {
        return;
    }

    // Convert to Angzarr types
    let new_node = Node::from_raw(new);
    let head_list = List::from_raw(head);

    // Call Angzarr's safe API
    head_list.insert_after_head(new_node);
}
```

### 3. Angzarr Core Execution

```rust
// angzarr-list/src/lib.rs

impl<T> List<T> {
    pub fn insert_after_head(&mut self, node: Node<T>) {
        // Safe Rust implementation
        match self.head {
            Some(head) => {
                node.next = Some(head);
                node.prev = None;
                head.prev = Some(node);
                self.head = Some(node);
            }
            None => {
                self.head = Some(node);
                self.tail = Some(node);
            }
        }
        self.len += 1;
    }
}
```

## Validation

### ABI Tests

```rust
// Tests verify adapter maintains compatibility
#[test]
fn test_list_add_compat() {
    unsafe {
        let mut head = list_head { next: null_mut(), prev: null_mut() };
        INIT_LIST_HEAD(&mut head);

        let mut node = list_head { next: null_mut(), prev: null_mut() };
        list_add(&mut node, &mut head);

        // Verify Linux behavior
        assert_eq!(head.next, &mut node as *mut _);
    }
}
```

### Core Tests

```rust
// Tests verify Rust API correctness
#[test]
fn test_list_insert() {
    let mut list = List::new();
    list.push_front(42);
    assert_eq!(list.len(), 1);
    assert_eq!(list.pop_front(), Some(42));
}
```

## Summary

The adapter layer architecture provides:

✅ **Clean separation** between Angzarr and Linux
✅ **Freedom** to optimize Angzarr internals
✅ **Perfect** Linux ABI compatibility
✅ **Safety** in core, unsafe isolated to adapters
✅ **Testability** of both APIs independently
✅ **Evolution** path without breaking changes

**Key Insight:** Linux compatibility is an interface requirement, not an architectural constraint. The adapter layer translates between the two worlds, giving us the best of both.
