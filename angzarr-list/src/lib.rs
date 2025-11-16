//! Linux kernel doubly-linked list implementation
//!
//! This module provides a Rust implementation of the Linux kernel's intrusive
//! doubly-linked list (`struct list_head`), maintaining binary compatibility
//! with C code.

#![cfg_attr(not(test), no_std)]

/// Intrusive doubly-linked list head
///
/// This is the Rust equivalent of Linux's `struct list_head`.
/// It must have identical memory layout for C compatibility.
#[repr(C)]
#[derive(Debug)]
pub struct ListHead {
    pub next: *mut ListHead,
    pub prev: *mut ListHead,
}

// Safety: ListHead is a raw pointer container used in kernel context
// where single-threaded or manually synchronized access is guaranteed
unsafe impl Send for ListHead {}
unsafe impl Sync for ListHead {}

impl ListHead {
    /// Create a new empty list head (points to itself)
    pub const fn new() -> Self {
        // We use a null pointer initially; it will be initialized properly
        Self {
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
        }
    }

    /// Initialize the list head to point to itself
    ///
    /// # Safety
    /// Must be called before using the list
    pub unsafe fn init(&mut self) {
        self.next = self as *mut ListHead;
        self.prev = self as *mut ListHead;
    }

    /// Check if list is empty
    pub fn is_empty(&self) -> bool {
        self.next == (self as *const ListHead as *mut ListHead)
    }

    /// Add a new entry after this head
    ///
    /// # Safety
    /// Caller must ensure `new` is a valid pointer and not already in a list
    pub unsafe fn add(&mut self, new: *mut ListHead) {
        let head_ptr = self as *mut ListHead;
        let next_ptr = self.next;
        Self::__list_add_raw(new, head_ptr, next_ptr);
    }

    /// Add a new entry before this head (at the tail)
    ///
    /// # Safety
    /// Caller must ensure `new` is a valid pointer and not already in a list
    pub unsafe fn add_tail(&mut self, new: *mut ListHead) {
        let head_ptr = self as *mut ListHead;
        let prev_ptr = self.prev;
        Self::__list_add_raw(new, prev_ptr, head_ptr);
    }

    /// Remove this entry from the list
    ///
    /// # Safety
    /// Caller must ensure this entry is in a list
    pub unsafe fn del(&mut self) {
        self.__list_del(self.prev, self.next);
        self.next = core::ptr::null_mut();
        self.prev = core::ptr::null_mut();
    }

    /// Delete entry from list and reinitialize it
    ///
    /// # Safety
    /// Caller must ensure this entry is in a list
    pub unsafe fn del_init(&mut self) {
        self.__list_del(self.prev, self.next);
        self.init();
    }

    /// Replace old entry with new entry
    ///
    /// # Safety
    /// Caller must ensure old is in a list and new is not
    pub unsafe fn replace(&mut self, new: *mut ListHead) {
        (*new).next = self.next;
        (*new).prev = self.prev;
        (*(*new).next).prev = new;
        (*(*new).prev).next = new;
    }

    /// Move this entry to the head of the list
    ///
    /// # Safety
    /// Caller must ensure entry is in a list and head is valid
    pub unsafe fn move_to_head(&mut self, head: *mut ListHead) {
        self.__list_del(self.prev, self.next);
        (*head).add(self as *mut ListHead);
    }

    /// Move this entry to the tail of the list
    ///
    /// # Safety
    /// Caller must ensure entry is in a list and head is valid
    pub unsafe fn move_to_tail(&mut self, head: *mut ListHead) {
        self.__list_del(self.prev, self.next);
        (*head).add_tail(self as *mut ListHead);
    }

    /// Internal helper to add entry between prev and next
    ///
    /// # Safety
    /// All pointers must be valid
    unsafe fn __list_add_raw(new: *mut ListHead, prev: *mut ListHead, next: *mut ListHead) {
        (*next).prev = new;
        (*new).next = next;
        (*new).prev = prev;
        (*prev).next = new;
    }

    /// Internal helper to delete entry
    ///
    /// # Safety
    /// All pointers must be valid
    unsafe fn __list_del(&mut self, prev: *mut ListHead, next: *mut ListHead) {
        (*next).prev = prev;
        (*prev).next = next;
    }
}

/// C-compatible FFI exports
#[no_mangle]
pub unsafe extern "C" fn INIT_LIST_HEAD(list: *mut ListHead) {
    (*list).init();
}

#[no_mangle]
pub unsafe extern "C" fn list_add(new: *mut ListHead, head: *mut ListHead) {
    (*head).add(new);
}

#[no_mangle]
pub unsafe extern "C" fn list_add_tail(new: *mut ListHead, head: *mut ListHead) {
    (*head).add_tail(new);
}

#[no_mangle]
pub unsafe extern "C" fn list_del(entry: *mut ListHead) {
    (*entry).del();
}

#[no_mangle]
pub unsafe extern "C" fn list_del_init(entry: *mut ListHead) {
    (*entry).del_init();
}

#[no_mangle]
pub unsafe extern "C" fn list_empty(head: *const ListHead) -> bool {
    (*head).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_init() {
        let mut head = ListHead::new();
        unsafe {
            head.init();
            assert!(head.is_empty());
            let head_ptr = &mut head as *mut ListHead;
            assert_eq!(head.next, head_ptr);
            assert_eq!(head.prev, head_ptr);
        }
    }

    #[test]
    fn test_list_add() {
        let mut head = ListHead::new();
        let mut entry1 = ListHead::new();
        let mut entry2 = ListHead::new();

        unsafe {
            head.init();
            entry1.init();
            entry2.init();

            head.add(&mut entry1 as *mut ListHead);
            assert!(!head.is_empty());
            assert_eq!(head.next, &mut entry1 as *mut ListHead);
            assert_eq!(entry1.prev, &mut head as *mut ListHead);

            head.add(&mut entry2 as *mut ListHead);
            assert_eq!(head.next, &mut entry2 as *mut ListHead);
            assert_eq!(entry2.next, &mut entry1 as *mut ListHead);
        }
    }

    #[test]
    fn test_list_add_tail() {
        let mut head = ListHead::new();
        let mut entry1 = ListHead::new();
        let mut entry2 = ListHead::new();

        unsafe {
            head.init();
            entry1.init();
            entry2.init();

            head.add_tail(&mut entry1 as *mut ListHead);
            assert_eq!(head.prev, &mut entry1 as *mut ListHead);

            head.add_tail(&mut entry2 as *mut ListHead);
            assert_eq!(head.prev, &mut entry2 as *mut ListHead);
            assert_eq!(entry1.next, &mut entry2 as *mut ListHead);
        }
    }

    #[test]
    fn test_list_del() {
        let mut head = ListHead::new();
        let mut entry = ListHead::new();

        unsafe {
            head.init();
            entry.init();

            head.add(&mut entry as *mut ListHead);
            assert!(!head.is_empty());

            entry.del();
            assert!(head.is_empty());
        }
    }

    #[test]
    fn test_list_del_init() {
        let mut head = ListHead::new();
        let mut entry = ListHead::new();

        unsafe {
            head.init();
            entry.init();

            head.add(&mut entry as *mut ListHead);
            entry.del_init();

            assert!(head.is_empty());
            assert!(entry.is_empty());
        }
    }

    #[test]
    fn test_list_replace() {
        let mut head = ListHead::new();
        let mut old = ListHead::new();
        let mut new = ListHead::new();

        unsafe {
            head.init();
            old.init();
            new.init();

            head.add(&mut old as *mut ListHead);
            old.replace(&mut new as *mut ListHead);

            assert_eq!(head.next, &mut new as *mut ListHead);
            assert_eq!(new.prev, &mut head as *mut ListHead);
        }
    }

    #[test]
    fn test_multiple_entries() {
        let mut head = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            head.init();
            e1.init();
            e2.init();
            e3.init();

            head.add_tail(&mut e1 as *mut ListHead);
            head.add_tail(&mut e2 as *mut ListHead);
            head.add_tail(&mut e3 as *mut ListHead);

            // Verify order: head -> e1 -> e2 -> e3 -> head
            assert_eq!(head.next, &mut e1 as *mut ListHead);
            assert_eq!(e1.next, &mut e2 as *mut ListHead);
            assert_eq!(e2.next, &mut e3 as *mut ListHead);
            assert_eq!(e3.next, &mut head as *mut ListHead);

            // Verify reverse order
            assert_eq!(head.prev, &mut e3 as *mut ListHead);
            assert_eq!(e3.prev, &mut e2 as *mut ListHead);
            assert_eq!(e2.prev, &mut e1 as *mut ListHead);
            assert_eq!(e1.prev, &mut head as *mut ListHead);
        }
    }
}

// C Reference Validation Tests
//
// These tests call C reference functions to get expected behavior
// and verify that Rust implementations match exactly.
//
// The C reference code is compiled from tests/c-reference/list/
// and linked into the test binary via build.rs.
#[cfg(test)]
#[cfg(c_reference)]
mod c_reference_tests {
    use super::*;
    use core::mem;

    // FFI declarations for C reference functions
    extern "C" {
        // Structure layout constants (extracted from C at compile time)
        static C_LIST_HEAD_SIZE: usize;
        static C_LIST_HEAD_ALIGN: usize;
        static C_LIST_HEAD_NEXT_OFFSET: usize;
        static C_LIST_HEAD_PREV_OFFSET: usize;

        // C reference functions (Linux-compatible behavior)
        fn c_ref_list_init(list: *mut ListHead);
        fn c_ref_list_add(new: *mut ListHead, head: *mut ListHead);
        fn c_ref_list_add_tail(new: *mut ListHead, head: *mut ListHead);
        fn c_ref_list_del(entry: *mut ListHead);
        fn c_ref_list_empty(head: *const ListHead) -> i32;
        fn c_ref_list_is_head(list: *const ListHead, head: *const ListHead) -> i32;
        fn c_ref_list_is_first(list: *const ListHead, head: *const ListHead) -> i32;
        fn c_ref_list_is_last(list: *const ListHead, head: *const ListHead) -> i32;
    }

    /// Test: Verify structure layout matches C
    #[test]
    fn test_c_layout_compatibility() {
        unsafe {
            // Dynamically extracted from C code
            assert_eq!(mem::size_of::<ListHead>(), C_LIST_HEAD_SIZE);
            assert_eq!(mem::align_of::<ListHead>(), C_LIST_HEAD_ALIGN);

            // Verify field offsets
            let dummy = ListHead {
                next: core::ptr::null_mut(),
                prev: core::ptr::null_mut(),
            };
            let base = &dummy as *const _ as usize;
            let next_offset = &dummy.next as *const _ as usize - base;
            let prev_offset = &dummy.prev as *const _ as usize - base;

            assert_eq!(next_offset, C_LIST_HEAD_NEXT_OFFSET);
            assert_eq!(prev_offset, C_LIST_HEAD_PREV_OFFSET);
        }
    }

    /// Test: Rust init() matches C behavior
    #[test]
    fn test_c_init_behavior() {
        unsafe {
            let mut c_list = ListHead::new();
            let mut rust_list = ListHead::new();

            // Get C expected behavior
            c_ref_list_init(&mut c_list as *mut ListHead);

            // Get Rust behavior
            rust_list.init();

            // Can't compare pointer values directly (different addresses)
            // But can verify both point to themselves (circular list)
            assert_eq!(c_list.next, &c_list as *const _ as *mut _);
            assert_eq!(c_list.prev, &c_list as *const _ as *mut _);
            assert_eq!(rust_list.next, &rust_list as *const _ as *mut _);
            assert_eq!(rust_list.prev, &rust_list as *const _ as *mut _);

            // Both should be empty
            assert_eq!(c_ref_list_empty(&c_list), 1);
            assert_eq!(rust_list.is_empty(), true);
        }
    }

    /// Test: Rust add() matches C behavior
    #[test]
    fn test_c_add_behavior() {
        unsafe {
            let mut c_head = ListHead::new();
            let mut c_entry1 = ListHead::new();
            let mut c_entry2 = ListHead::new();

            let mut rust_head = ListHead::new();
            let mut rust_entry1 = ListHead::new();
            let mut rust_entry2 = ListHead::new();

            // Setup C list
            c_ref_list_init(&mut c_head);
            c_ref_list_init(&mut c_entry1);
            c_ref_list_init(&mut c_entry2);
            c_ref_list_add(&mut c_entry1, &mut c_head);
            c_ref_list_add(&mut c_entry2, &mut c_head);

            // Setup Rust list (same operations)
            rust_head.init();
            rust_entry1.init();
            rust_entry2.init();
            rust_head.add(&mut rust_entry1);
            rust_head.add(&mut rust_entry2);

            // Verify same structure: head -> entry2 -> entry1 -> head
            let c_next1 = c_head.next;
            let c_next2 = (c_next1 as *const ListHead).read().next;

            let rust_next1 = rust_head.next;
            let rust_next2 = (rust_next1 as *const ListHead).read().next;

            // Can't compare pointers directly (different addresses)
            // But can verify same relative structure
            assert_eq!(c_next1, &c_entry2 as *const _ as *mut _);
            assert_eq!(c_next2, &c_entry1 as *const _ as *mut _);
            assert_eq!(rust_next1, &rust_entry2 as *const _ as *mut _);
            assert_eq!(rust_next2, &rust_entry1 as *const _ as *mut _);
        }
    }

    /// Test: Rust add_tail() matches C behavior
    #[test]
    fn test_c_add_tail_behavior() {
        unsafe {
            let mut c_head = ListHead::new();
            let mut c_entry1 = ListHead::new();
            let mut c_entry2 = ListHead::new();

            let mut rust_head = ListHead::new();
            let mut rust_entry1 = ListHead::new();
            let mut rust_entry2 = ListHead::new();

            // Setup C list
            c_ref_list_init(&mut c_head);
            c_ref_list_add_tail(&mut c_entry1, &mut c_head);
            c_ref_list_add_tail(&mut c_entry2, &mut c_head);

            // Setup Rust list
            rust_head.init();
            rust_head.add_tail(&mut rust_entry1);
            rust_head.add_tail(&mut rust_entry2);

            // Verify same structure: head -> entry1 -> entry2 -> head
            assert_eq!(c_head.next, &c_entry1 as *const _ as *mut _);
            assert_eq!(rust_head.next, &rust_entry1 as *const _ as *mut _);

            let c_next2 = c_entry1.next;
            let rust_next2 = rust_entry1.next;
            assert_eq!(c_next2, &c_entry2 as *const _ as *mut _);
            assert_eq!(rust_next2, &rust_entry2 as *const _ as *mut _);
        }
    }

    /// Test: Rust del() matches C behavior
    #[test]
    fn test_c_del_behavior() {
        unsafe {
            let mut c_head = ListHead::new();
            let mut c_entry = ListHead::new();

            let mut rust_head = ListHead::new();
            let mut rust_entry = ListHead::new();

            // Setup C list
            c_ref_list_init(&mut c_head);
            c_ref_list_add(&mut c_entry, &mut c_head);
            c_ref_list_del(&mut c_entry);

            // Setup Rust list
            rust_head.init();
            rust_head.add(&mut rust_entry);
            rust_entry.del();

            // After deletion, both should have empty list
            assert_eq!(c_ref_list_empty(&c_head), 1);
            assert_eq!(rust_head.is_empty(), true);

            // Entry pointers should be null after del
            assert_eq!(c_entry.next, core::ptr::null_mut());
            assert_eq!(c_entry.prev, core::ptr::null_mut());
            assert_eq!(rust_entry.next, core::ptr::null_mut());
            assert_eq!(rust_entry.prev, core::ptr::null_mut());
        }
    }

    /// Test: Rust is_empty() matches C behavior
    #[test]
    fn test_c_empty_behavior() {
        unsafe {
            let mut c_head = ListHead::new();
            let mut c_entry = ListHead::new();

            let mut rust_head = ListHead::new();
            let mut rust_entry = ListHead::new();

            c_ref_list_init(&mut c_head);
            rust_head.init();

            // Both empty initially
            assert_eq!(c_ref_list_empty(&c_head), 1);
            assert_eq!(rust_head.is_empty(), true);

            // Add entry
            c_ref_list_add(&mut c_entry, &mut c_head);
            rust_head.add(&mut rust_entry);

            // Both non-empty
            assert_eq!(c_ref_list_empty(&c_head), 0);
            assert_eq!(rust_head.is_empty(), false);

            // Delete entry
            c_ref_list_del(&mut c_entry);
            rust_entry.del();

            // Both empty again
            assert_eq!(c_ref_list_empty(&c_head), 1);
            assert_eq!(rust_head.is_empty(), true);
        }
    }

    /// Test: List position checks match C
    #[test]
    fn test_c_position_checks() {
        unsafe {
            let mut head = ListHead::new();
            let mut e1 = ListHead::new();
            let mut e2 = ListHead::new();
            let mut e3 = ListHead::new();

            c_ref_list_init(&mut head);
            c_ref_list_add_tail(&mut e1, &mut head);
            c_ref_list_add_tail(&mut e2, &mut head);
            c_ref_list_add_tail(&mut e3, &mut head);

            // Test is_head
            assert_eq!(c_ref_list_is_head(&head, &head), 1);
            assert_eq!(c_ref_list_is_head(&e1, &head), 0);

            // Test is_first
            assert_eq!(c_ref_list_is_first(&e1, &head), 1);
            assert_eq!(c_ref_list_is_first(&e2, &head), 0);
            assert_eq!(c_ref_list_is_first(&e3, &head), 0);

            // Test is_last
            assert_eq!(c_ref_list_is_last(&e3, &head), 1);
            assert_eq!(c_ref_list_is_last(&e1, &head), 0);
            assert_eq!(c_ref_list_is_last(&e2, &head), 0);
        }
    }
}
