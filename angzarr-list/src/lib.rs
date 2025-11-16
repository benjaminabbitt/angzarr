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
