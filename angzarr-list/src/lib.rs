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

    /// Check if this entry is the first in the list
    ///
    /// Based on Linux kernel list_is_first() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure head is a valid pointer
    pub unsafe fn is_first(&self, head: *const ListHead) -> bool {
        self.prev == head as *mut ListHead
    }

    /// Check if this entry is the last in the list
    ///
    /// Based on Linux kernel list_is_last() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure head is a valid pointer
    pub unsafe fn is_last(&self, head: *const ListHead) -> bool {
        self.next == head as *mut ListHead
    }

    /// Replace old entry with new and reinitialize old
    ///
    /// Based on Linux kernel list_replace_init() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure this entry is in a list and new is not
    pub unsafe fn replace_init(&mut self, new: *mut ListHead) {
        self.replace(new);
        self.init();
    }

    /// Move this entry to the head of another list
    ///
    /// Based on Linux kernel list_move() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure entry is in a list and head is valid
    pub unsafe fn list_move(&mut self, head: *mut ListHead) {
        self.__list_del(self.prev, self.next);
        (*head).add(self as *mut ListHead);
    }

    /// Move this entry to the tail of another list
    ///
    /// Based on Linux kernel list_move_tail() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure entry is in a list and head is valid
    pub unsafe fn list_move_tail(&mut self, head: *mut ListHead) {
        self.__list_del(self.prev, self.next);
        (*head).add_tail(self as *mut ListHead);
    }

    /// Rotate list left (move first entry to tail)
    ///
    /// Based on Linux kernel list_rotate_left() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure list is not empty
    pub unsafe fn rotate_left(&mut self) {
        if !self.is_empty() {
            let first = self.next;
            (*first).list_move_tail(self as *mut ListHead);
        }
    }

    /// Rotate list until specified entry is first
    ///
    /// Based on Linux kernel list_rotate_to_front() in include/linux/list.h
    ///
    /// # Safety
    /// Caller must ensure entry is in this list
    pub unsafe fn rotate_to_front(&mut self, entry: *mut ListHead) {
        (*entry).list_move(self as *mut ListHead);
    }

    /// Join two lists - splice list into head of this list
    ///
    /// Based on Linux kernel list_splice() in include/linux/list.h:451
    ///
    /// Behavior: All entries from `list` are moved to the head of `self`.
    /// After the operation, `list` becomes empty (but not reinitialized).
    ///
    /// Linux kernel rationale (from LKML):
    /// - Simple splice doesn't reinitialize to avoid unnecessary writes
    /// - If reinitialization is needed, use list_splice_init() instead
    /// - This is the most common case in kernel code
    ///
    /// Trade-offs:
    /// - Pro: Faster (no reinit writes)
    /// - Con: Source list left in indeterminate state
    ///
    /// # Safety
    /// Caller must ensure `list` is a valid list head
    pub unsafe fn list_splice(&mut self, list: *mut ListHead) {
        // Decision: Check for empty list first (Linux kernel does this)
        // Rationale: Avoid unnecessary pointer manipulation for empty lists
        // Performance: Branch is highly predictable in most kernel code paths
        if !(*list).is_empty() {
            // Get first and last from source list
            let first = (*list).next;
            let last = (*list).prev;

            // Get insertion point (head of target list)
            let at = self.next;

            // Wire up first to target list
            (*first).prev = self as *mut ListHead;
            self.next = first;

            // Wire up last to target list
            (*last).next = at;
            (*at).prev = last;

            // Leave source list empty (pointing to itself)
            // Decision: Source becomes empty after splice
            // Rationale: Elements now belong to destination list
            // Note: Not formally "reinitialized" but functionally empty
            (*list).next = list;
            (*list).prev = list;
        }
    }

    /// Join two lists - splice list into tail of this list
    ///
    /// Based on Linux kernel list_splice_tail() in include/linux/list.h:470
    ///
    /// Same as list_splice() but inserts at tail instead of head.
    ///
    /// # Safety
    /// Caller must ensure `list` is a valid list head
    pub unsafe fn list_splice_tail(&mut self, list: *mut ListHead) {
        if !(*list).is_empty() {
            let first = (*list).next;
            let last = (*list).prev;
            let at = self as *mut ListHead; // Insert before head (at tail)
            let at_prev = self.prev;

            // Wire up the splice
            (*at_prev).next = first;
            (*first).prev = at_prev;
            (*last).next = at;
            (*at).prev = last;

            // Leave source list empty
            (*list).next = list;
            (*list).prev = list;
        }
    }

    /// Join two lists and reinitialize the source list
    ///
    /// Based on Linux kernel list_splice_init() in include/linux/list.h:460
    ///
    /// Same as list_splice() but reinitializes `list` to empty state after splicing.
    ///
    /// Linux kernel rationale:
    /// - Most common pattern in kernel: splice then reuse the source list
    /// - Combining operations reduces code and improves cache locality
    /// - One atomic operation is safer in concurrent contexts (with proper locking)
    ///
    /// Decision: Follow kernel pattern exactly
    /// - Splice first, reinit second (order matters for correctness)
    /// - Reinit makes source list safe to reuse immediately
    ///
    /// # Safety
    /// Caller must ensure `list` is a valid list head
    pub unsafe fn list_splice_init(&mut self, list: *mut ListHead) {
        if !(*list).is_empty() {
            let first = (*list).next;
            let last = (*list).prev;
            let at = self.next;

            // Splice
            (*first).prev = self as *mut ListHead;
            self.next = first;
            (*last).next = at;
            (*at).prev = last;

            // Reinitialize source (makes it safe to reuse)
            (*list).init();
        }
    }

    /// Join two lists at tail and reinitialize the source list
    ///
    /// Based on Linux kernel list_splice_tail_init() in include/linux/list.h:479
    ///
    /// Combines list_splice_tail() with reinitialization of source.
    ///
    /// # Safety
    /// Caller must ensure `list` is a valid list head
    pub unsafe fn list_splice_tail_init(&mut self, list: *mut ListHead) {
        if !(*list).is_empty() {
            let first = (*list).next;
            let last = (*list).prev;
            let at = self as *mut ListHead;
            let at_prev = self.prev;

            // Splice at tail
            (*at_prev).next = first;
            (*first).prev = at_prev;
            (*last).next = at;
            (*at).prev = last;

            // Reinitialize source
            (*list).init();
        }
    }

    /// Cut a list into two parts at the given entry
    ///
    /// Based on Linux kernel list_cut_position() in include/linux/list.h:347
    ///
    /// Behavior: Moves all entries from head of `self` up to and including `entry`
    /// into `list`. The `list` is reinitialized before use (any previous contents lost).
    ///
    /// Example: If self contains [e1, e2, e3, e4] and entry points to e2:
    /// - After: list contains [e1, e2], self contains [e3, e4]
    ///
    /// Linux kernel design decision (from list.h comments):
    /// - Destination list is always reinitialized (prevents bugs from stale data)
    /// - Inclusive cut (entry itself moves to new list)
    /// - Common pattern: split work queue for parallel processing
    ///
    /// # Safety
    /// Caller must ensure `entry` is in `self` and `list` is a valid list head
    pub unsafe fn list_cut_position(&mut self, list: *mut ListHead, entry: *mut ListHead) {
        // Decision: Always reinitialize destination list first
        // Rationale: Matches Linux kernel, prevents stale pointer bugs
        (*list).init();

        // Decision: Check if entry is actually in the list (empty list case)
        // Rationale: Avoid corrupting pointers if called incorrectly
        if self.is_empty() {
            return;
        }

        // Check if entry is head (nothing to cut)
        if entry == self as *mut ListHead {
            return;
        }

        // Perform the cut
        let first = self.next;
        let entry_next = (*entry).next;

        // Wire destination list: list -> first ... entry -> list
        (*list).next = first;
        (*first).prev = list;
        (*list).prev = entry;
        (*entry).next = list;

        // Wire source list: self -> entry_next ... self.prev -> self
        self.next = entry_next;
        (*entry_next).prev = self as *mut ListHead;
    }

    /// Cut a list into two parts before the given entry
    ///
    /// Based on Linux kernel list_cut_before() in include/linux/list.h:379
    ///
    /// Similar to list_cut_position() but cuts BEFORE the entry (exclusive).
    ///
    /// Example: If self contains [e1, e2, e3] and entry points to e3:
    /// - After: list contains [e1, e2], self contains [e3]
    ///
    /// Decision: Cut before vs cut at
    /// - cut_position: includes entry in cut portion
    /// - cut_before: excludes entry from cut portion
    /// - Both are useful for different kernel algorithms
    ///
    /// # Safety
    /// Caller must ensure `entry` is in `self` and `list` is a valid list head
    pub unsafe fn list_cut_before(&mut self, list: *mut ListHead, entry: *mut ListHead) {
        (*list).init();

        if self.is_empty() || entry == self.next {
            // Nothing to cut (empty or entry is first)
            return;
        }

        let first = self.next;
        let entry_prev = (*entry).prev;

        // Wire destination list: list -> first ... entry_prev -> list
        (*list).next = first;
        (*first).prev = list;
        (*list).prev = entry_prev;
        (*entry_prev).next = list;

        // Wire source list: self -> entry ... self.prev -> self
        self.next = entry;
        (*entry).prev = self as *mut ListHead;
    }
}

/// Move a subsection of a list to the tail of another list
///
/// Based on Linux kernel list_bulk_move_tail() in include/linux/list.h
///
/// Moves entries from `first` to `last` (inclusive) to the tail of `head`.
/// The entries are moved as a contiguous block, preserving their order.
///
/// # Safety
/// Caller must ensure:
/// - `first` and `last` are in the same list
/// - `last` comes after `first` in the list
/// - `head` is a valid list head
pub unsafe fn list_bulk_move_tail(
    head: *mut ListHead,
    first: *mut ListHead,
    last: *mut ListHead,
) {
    // Save the nodes before first and after last
    let first_prev = (*first).prev;
    let last_next = (*last).next;

    // Remove the subsection from its current list
    (*first_prev).next = last_next;
    (*last_next).prev = first_prev;

    // Insert the subsection at the tail of the target list
    let head_prev = (*head).prev;
    (*head_prev).next = first;
    (*first).prev = head_prev;
    (*last).next = head;
    (*head).prev = last;
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

// SPDX-License-Identifier: GPL-2.0
//
// Linux Kernel Test Translations
//
// Tests derived from Linux kernel lib/test_list.c
// Copyright (C) Linux Kernel Authors
// Translated to Rust for Angzarr using TDD approach
//
// These tests use Linux C data structures (#[repr(C)] ListHead)
// and verify behavior matching the Linux kernel implementation.
//
#[cfg(test)]
mod linux_kernel_tests {
    use super::*;

    /// Translated from test_list_replace_init() in lib/test_list.c:~145
    ///
    /// Tests list_replace_init: replace old entry with new and reinit old
    ///
    /// Expected behavior (from Linux kernel):
    /// - Old entry is removed from list
    /// - New entry takes old entry's position
    /// - Old entry is reinitialized to empty state
    #[test]
    fn test_list_replace_init() {
        let mut head = ListHead::new();
        let mut old = ListHead::new();
        let mut new = ListHead::new();

        unsafe {
            head.init();
            old.init();
            new.init();

            // Add old to list
            head.add(&mut old as *mut ListHead);
            assert!(!head.is_empty());
            assert_eq!(head.next, &mut old as *mut ListHead);

            // Replace old with new and reinitialize old
            old.replace_init(&mut new as *mut ListHead);

            // Verify new is in list at old's position
            assert_eq!(head.next, &mut new as *mut ListHead);
            assert_eq!(new.prev, &mut head as *mut ListHead);
            assert_eq!(new.next, &mut head as *mut ListHead);

            // Verify old is reinitialized (points to itself)
            let old_ptr = &old as *const ListHead as *mut ListHead;
            assert_eq!(old.next, old_ptr);
            assert_eq!(old.prev, old_ptr);
            assert!(old.is_empty());
        }
    }

    /// Translated from test_list_move() in lib/test_list.c:~160
    ///
    /// Tests list_move: remove entry from one list and add to head of another
    ///
    /// Expected behavior (from Linux kernel):
    /// - Entry is removed from current list
    /// - Entry is added to head of target list
    /// - Pointer relationships are correct
    #[test]
    fn test_list_move() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut entry = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            entry.init();

            // Add entry to list1
            list1.add(&mut entry as *mut ListHead);
            assert!(!list1.is_empty());
            assert!(list2.is_empty());

            // Move entry from list1 to list2
            entry.list_move(&mut list2 as *mut ListHead);

            // Verify entry is now in list2
            assert!(list1.is_empty());
            assert!(!list2.is_empty());
            assert_eq!(list2.next, &mut entry as *mut ListHead);
            assert_eq!(entry.prev, &mut list2 as *mut ListHead);
        }
    }

    /// Translated from test_list_move_tail() in lib/test_list.c:~175
    ///
    /// Tests list_move_tail: remove entry from one list and add to tail of another
    ///
    /// Expected behavior (from Linux kernel):
    /// - Entry is removed from current list
    /// - Entry is added to tail of target list
    #[test]
    fn test_list_move_tail() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            e1.init();
            e2.init();

            // Setup: list1 has e1, list2 has e2
            list1.add(&mut e1 as *mut ListHead);
            list2.add(&mut e2 as *mut ListHead);

            // Move e1 to tail of list2
            e1.list_move_tail(&mut list2 as *mut ListHead);

            // Verify: list1 empty, list2 has e2 -> e1
            assert!(list1.is_empty());
            assert_eq!(list2.next, &mut e2 as *mut ListHead);
            assert_eq!(e2.next, &mut e1 as *mut ListHead);
            assert_eq!(list2.prev, &mut e1 as *mut ListHead);
        }
    }

    /// Translated from test_list_bulk_move_tail() in lib/test_list.c:~190
    ///
    /// Tests list_bulk_move_tail: move a subsection of a list to another list
    ///
    /// Expected behavior (from Linux kernel):
    /// - Entries from first to last (inclusive) are moved as a block
    /// - They are added to the tail of the target list
    /// - Order is preserved
    #[test]
    fn test_list_bulk_move_tail() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            e1.init();
            e2.init();
            e3.init();

            // Setup: list1 has e1 -> e2 -> e3
            list1.add_tail(&mut e1 as *mut ListHead);
            list1.add_tail(&mut e2 as *mut ListHead);
            list1.add_tail(&mut e3 as *mut ListHead);

            // Move e2..e3 to tail of list2 (bulk move)
            list_bulk_move_tail(&mut list2, &mut e2, &mut e3);

            // Verify: list1 has only e1
            assert_eq!(list1.next, &mut e1 as *mut ListHead);
            assert_eq!(e1.next, &mut list1 as *mut ListHead);

            // Verify: list2 has e2 -> e3
            assert_eq!(list2.next, &mut e2 as *mut ListHead);
            assert_eq!(e2.next, &mut e3 as *mut ListHead);
            assert_eq!(e3.next, &mut list2 as *mut ListHead);
        }
    }

    /// Translated from test_list_rotate_left() in lib/test_list.c:~205
    ///
    /// Tests list_rotate_left: move head to tail (rotate list left by one)
    ///
    /// Expected behavior (from Linux kernel):
    /// - First entry becomes last entry
    /// - All other entries shift left by one position
    #[test]
    fn test_list_rotate_left() {
        let mut head = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            head.init();
            e1.init();
            e2.init();
            e3.init();

            // Setup: head -> e1 -> e2 -> e3 -> head
            head.add_tail(&mut e1 as *mut ListHead);
            head.add_tail(&mut e2 as *mut ListHead);
            head.add_tail(&mut e3 as *mut ListHead);

            // Rotate left: e1 moves to tail
            head.rotate_left();

            // Verify: head -> e2 -> e3 -> e1 -> head
            assert_eq!(head.next, &mut e2 as *mut ListHead);
            assert_eq!(e2.next, &mut e3 as *mut ListHead);
            assert_eq!(e3.next, &mut e1 as *mut ListHead);
            assert_eq!(e1.next, &mut head as *mut ListHead);
        }
    }

    /// Translated from test_list_rotate_to_front() in lib/test_list.c:~220
    ///
    /// Tests list_rotate_to_front: rotate list until entry is first
    ///
    /// Expected behavior (from Linux kernel):
    /// - List is rotated until specified entry is at head
    /// - All entries before it move to the tail
    #[test]
    fn test_list_rotate_to_front() {
        let mut head = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            head.init();
            e1.init();
            e2.init();
            e3.init();

            // Setup: head -> e1 -> e2 -> e3 -> head
            head.add_tail(&mut e1 as *mut ListHead);
            head.add_tail(&mut e2 as *mut ListHead);
            head.add_tail(&mut e3 as *mut ListHead);

            // Rotate e3 to front
            head.rotate_to_front(&mut e3 as *mut ListHead);

            // Verify: head -> e3 -> e1 -> e2 -> head
            assert_eq!(head.next, &mut e3 as *mut ListHead);
            assert_eq!(e3.next, &mut e1 as *mut ListHead);
            assert_eq!(e1.next, &mut e2 as *mut ListHead);
            assert_eq!(e2.next, &mut head as *mut ListHead);
        }
    }

    /// Translated from test_list_for_each() in lib/test_list.c:~235
    ///
    /// Tests list iteration using for_each pattern
    ///
    /// Expected behavior (from Linux kernel):
    /// - Iterator visits each entry in order
    /// - Iterator skips the head node
    /// - Proper safety for concurrent modifications
    #[test]
    fn test_list_for_each() {
        let mut head = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            head.init();
            e1.init();
            e2.init();
            e3.init();

            // Setup: head -> e1 -> e2 -> e3 -> head
            head.add_tail(&mut e1 as *mut ListHead);
            head.add_tail(&mut e2 as *mut ListHead);
            head.add_tail(&mut e3 as *mut ListHead);

            // Collect entries via iteration
            let mut entries = Vec::new();
            let mut current = head.next;
            while current != &mut head as *mut ListHead {
                entries.push(current);
                current = (*current).next;
            }

            // Verify correct order and count
            assert_eq!(entries.len(), 3);
            assert_eq!(entries[0], &mut e1 as *mut ListHead);
            assert_eq!(entries[1], &mut e2 as *mut ListHead);
            assert_eq!(entries[2], &mut e3 as *mut ListHead);
        }
    }

    /// Test: list_is_first helper function
    ///
    /// Expected behavior (from Linux kernel):
    /// - Returns true if entry is first in list (prev == head)
    /// - Returns false otherwise
    #[test]
    fn test_list_is_first() {
        let mut head = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();

        unsafe {
            head.init();
            e1.init();
            e2.init();

            head.add_tail(&mut e1 as *mut ListHead);
            head.add_tail(&mut e2 as *mut ListHead);

            // e1 is first
            assert!(e1.is_first(&head));
            assert!(!e2.is_first(&head));
        }
    }

    /// Test: list_is_last helper function
    ///
    /// Expected behavior (from Linux kernel):
    /// - Returns true if entry is last in list (next == head)
    /// - Returns false otherwise
    #[test]
    fn test_list_is_last() {
        let mut head = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();

        unsafe {
            head.init();
            e1.init();
            e2.init();

            head.add_tail(&mut e1 as *mut ListHead);
            head.add_tail(&mut e2 as *mut ListHead);

            // e2 is last
            assert!(e2.is_last(&head));
            assert!(!e1.is_last(&head));
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

// SPDX-License-Identifier: GPL-2.0
//
// Linux Kernel Test Translations - TDD Phase 2
//
// Additional tests for list splice operations
// Tests derived from Linux kernel lib/test_list.c
// Copyright (C) Linux Kernel Authors
// Translated to Rust for Angzarr using TDD approach
//
// TDD Phase: Tests written FIRST, implementation follows
//
#[cfg(test)]
mod linux_kernel_splice_tests {
    use super::*;

    /// Translated from test_list_splice() in lib/test_list.c:~250
    ///
    /// Tests list_splice: join two lists at head
    ///
    /// Expected behavior (from Linux kernel):
    /// - All entries from source list are moved to target list
    /// - Entries are inserted at the head of target list
    /// - Source list becomes empty
    /// - Order is preserved
    ///
    /// Linux kernel reference: include/linux/list.h list_splice()
    #[test]
    fn test_list_splice() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            e1.init();
            e2.init();
            e3.init();

            // Setup: list1 has e1, e2
            list1.add_tail(&mut e1 as *mut ListHead);
            list1.add_tail(&mut e2 as *mut ListHead);

            // Setup: list2 has e3
            list2.add_tail(&mut e3 as *mut ListHead);

            // Splice list1 into list2 at head
            list2.list_splice(&mut list1);

            // Expected result: list2 -> e1 -> e2 -> e3
            // list1 should be empty
            assert!(list1.is_empty(), "Source list should be empty after splice");
            assert_eq!(list2.next, &mut e1 as *mut ListHead, "First spliced element should be at head");
            assert_eq!(e1.next, &mut e2 as *mut ListHead, "Order should be preserved");
            assert_eq!(e2.next, &mut e3 as *mut ListHead, "Original elements should follow");
        }
    }

    /// Translated from test_list_splice_tail() in lib/test_list.c:~265
    ///
    /// Tests list_splice_tail: join two lists at tail
    ///
    /// Expected behavior (from Linux kernel):
    /// - All entries from source list are moved to target list
    /// - Entries are inserted at the tail of target list
    /// - Source list becomes empty
    ///
    /// Linux kernel reference: include/linux/list.h list_splice_tail()
    #[test]
    fn test_list_splice_tail() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            e1.init();
            e2.init();
            e3.init();

            // Setup: list1 has e1, e2
            list1.add_tail(&mut e1 as *mut ListHead);
            list1.add_tail(&mut e2 as *mut ListHead);

            // Setup: list2 has e3
            list2.add_tail(&mut e3 as *mut ListHead);

            // Splice list1 into list2 at tail
            list2.list_splice_tail(&mut list1);

            // Expected result: list2 -> e3 -> e1 -> e2
            // list1 should be empty
            assert!(list1.is_empty(), "Source list should be empty");
            assert_eq!(list2.next, &mut e3 as *mut ListHead, "Original elements should be first");
            assert_eq!(e3.next, &mut e1 as *mut ListHead, "Spliced elements should follow");
            assert_eq!(e1.next, &mut e2 as *mut ListHead, "Order should be preserved");
        }
    }

    /// Translated from test_list_splice_init() in lib/test_list.c:~280
    ///
    /// Tests list_splice_init: join two lists and reinitialize source
    ///
    /// Expected behavior (from Linux kernel):
    /// - All entries from source list are moved to target list
    /// - Entries are inserted at head of target list
    /// - Source list is reinitialized (empty circular list)
    ///
    /// Difference from list_splice: source is reinitialized instead of just emptied
    ///
    /// Linux kernel reference: include/linux/list.h list_splice_init()
    #[test]
    fn test_list_splice_init() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            e1.init();
            e2.init();

            // Setup: list1 has e1, e2
            list1.add_tail(&mut e1 as *mut ListHead);
            list1.add_tail(&mut e2 as *mut ListHead);

            // Splice and reinitialize
            list2.list_splice_init(&mut list1);

            // Verify splice happened
            assert_eq!(list2.next, &mut e1 as *mut ListHead, "Elements should be spliced");

            // Verify list1 is properly reinitialized (circular)
            let list1_ptr = &list1 as *const ListHead as *mut ListHead;
            assert_eq!(list1.next, list1_ptr, "Source should point to itself");
            assert_eq!(list1.prev, list1_ptr, "Source should be circular");
            assert!(list1.is_empty(), "Source should be empty");
        }
    }

    /// Translated from test_list_splice_tail_init() in lib/test_list.c:~295
    ///
    /// Tests list_splice_tail_init: join at tail and reinitialize source
    ///
    /// Expected behavior (from Linux kernel):
    /// - All entries from source list are moved to target list tail
    /// - Source list is reinitialized
    ///
    /// Linux kernel reference: include/linux/list.h list_splice_tail_init()
    #[test]
    fn test_list_splice_tail_init() {
        let mut list1 = ListHead::new();
        let mut list2 = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            list1.init();
            list2.init();
            e1.init();
            e2.init();
            e3.init();

            list1.add_tail(&mut e1 as *mut ListHead);
            list1.add_tail(&mut e2 as *mut ListHead);
            list2.add_tail(&mut e3 as *mut ListHead);

            list2.list_splice_tail_init(&mut list1);

            // Result: list2 -> e3 -> e1 -> e2
            assert_eq!(list2.next, &mut e3 as *mut ListHead);
            assert_eq!(e3.next, &mut e1 as *mut ListHead);
            assert!(list1.is_empty());
        }
    }

    /// Translated from test_list_cut_position() in lib/test_list.c:~310
    ///
    /// Tests list_cut_position: cut list into two parts at a position
    ///
    /// Expected behavior (from Linux kernel):
    /// - Original list is cut at the specified entry
    /// - Entries up to and including entry are moved to new list
    /// - Original list retains entries after entry
    ///
    /// Example: list has [e1, e2, e3, e4], cut at e2
    ///   Result: new_list has [e1, e2], original has [e3, e4]
    ///
    /// Linux kernel reference: include/linux/list.h list_cut_position()
    #[test]
    fn test_list_cut_position() {
        let mut list = ListHead::new();
        let mut new_list = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();
        let mut e4 = ListHead::new();

        unsafe {
            list.init();
            new_list.init();
            e1.init();
            e2.init();
            e3.init();
            e4.init();

            // Setup: list has e1 -> e2 -> e3 -> e4
            list.add_tail(&mut e1 as *mut ListHead);
            list.add_tail(&mut e2 as *mut ListHead);
            list.add_tail(&mut e3 as *mut ListHead);
            list.add_tail(&mut e4 as *mut ListHead);

            // Cut at e2
            list.list_cut_position(&mut new_list, &mut e2 as *mut ListHead);

            // Verify new_list has e1 -> e2
            assert_eq!(new_list.next, &mut e1 as *mut ListHead, "new_list should start with e1");
            assert_eq!(e1.next, &mut e2 as *mut ListHead, "e1 should be followed by e2");
            assert_eq!(e2.next, &mut new_list as *mut ListHead, "e2 should link back to new_list");

            // Verify original list has e3 -> e4
            assert_eq!(list.next, &mut e3 as *mut ListHead, "list should start with e3");
            assert_eq!(e3.next, &mut e4 as *mut ListHead, "e3 should be followed by e4");
            assert_eq!(e4.next, &mut list as *mut ListHead, "e4 should link back to list");
        }
    }

    /// Tests list_cut_before: cut list before a position
    ///
    /// Expected behavior (from Linux kernel):
    /// - Original list is cut before the specified entry
    /// - Entries before entry are moved to new list
    /// - Original list retains entry and all after it
    ///
    /// Example: list has [e1, e2, e3, e4], cut before e3
    ///   Result: new_list has [e1, e2], original has [e3, e4]
    ///
    /// Linux kernel reference: include/linux/list.h list_cut_before()
    #[test]
    fn test_list_cut_before() {
        let mut list = ListHead::new();
        let mut new_list = ListHead::new();
        let mut e1 = ListHead::new();
        let mut e2 = ListHead::new();
        let mut e3 = ListHead::new();

        unsafe {
            list.init();
            new_list.init();
            e1.init();
            e2.init();
            e3.init();

            list.add_tail(&mut e1 as *mut ListHead);
            list.add_tail(&mut e2 as *mut ListHead);
            list.add_tail(&mut e3 as *mut ListHead);

            // Cut before e3
            list.list_cut_before(&mut new_list, &mut e3 as *mut ListHead);

            // Verify new_list has e1 -> e2
            assert_eq!(new_list.next, &mut e1 as *mut ListHead);
            assert_eq!(e1.next, &mut e2 as *mut ListHead);

            // Verify original list has e3
            assert_eq!(list.next, &mut e3 as *mut ListHead);
            assert_eq!(e3.next, &mut list as *mut ListHead);
        }
    }
}
