//! Linux-compatible list API adapter
//!
//! Provides Linux `struct list_head` compatible API that translates
//! to Angzarr's internal list implementation.

use core::ptr;

/// Linux-compatible list_head structure
///
/// This matches the Linux kernel's `struct list_head` exactly:
/// ```c
/// struct list_head {
///     struct list_head *next, *prev;
/// };
/// ```
///
/// Binary layout is verified by ABI compatibility tests.
#[repr(C)]
#[derive(Debug)]
pub struct list_head {
    pub next: *mut list_head,
    pub prev: *mut list_head,
}

// SAFETY: list_head is a POD type containing only pointers
unsafe impl Send for list_head {}
unsafe impl Sync for list_head {}

/// Initialize a list head to point to itself
///
/// Linux equivalent: `INIT_LIST_HEAD(ptr)`
///
/// # Safety
///
/// Caller must ensure `list` is a valid, non-null pointer to list_head.
#[no_mangle]
pub unsafe extern "C" fn INIT_LIST_HEAD(list: *mut list_head) {
    if list.is_null() {
        return;
    }

    (*list).next = list;
    (*list).prev = list;
}

/// Add a new entry after the specified head
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
    if new.is_null() || head.is_null() {
        return;
    }

    __list_add(new, head, (*head).next);
}

/// Add a new entry before the specified head (at tail)
///
/// Linux equivalent: `list_add_tail(new, head)`
///
/// Insert a new entry before the specified head.
/// This is useful for implementing queues.
///
/// # Safety
///
/// - Both `new` and `head` must be valid, non-null pointers
/// - `new` must not already be in a list
#[no_mangle]
pub unsafe extern "C" fn list_add_tail(new: *mut list_head, head: *mut list_head) {
    if new.is_null() || head.is_null() {
        return;
    }

    __list_add(new, (*head).prev, head);
}

/// Delete an entry from list
///
/// Linux equivalent: `list_del(entry)`
///
/// # Safety
///
/// `entry` must be valid and currently in a list
#[no_mangle]
pub unsafe extern "C" fn list_del(entry: *mut list_head) {
    if entry.is_null() {
        return;
    }

    __list_del((*entry).prev, (*entry).next);
    (*entry).next = ptr::null_mut();
    (*entry).prev = ptr::null_mut();
}

/// Delete an entry and reinitialize it
///
/// Linux equivalent: `list_del_init(entry)`
///
/// # Safety
///
/// `entry` must be valid and currently in a list
#[no_mangle]
pub unsafe extern "C" fn list_del_init(entry: *mut list_head) {
    if entry.is_null() {
        return;
    }

    __list_del((*entry).prev, (*entry).next);
    INIT_LIST_HEAD(entry);
}

/// Test whether a list is empty
///
/// Linux equivalent: `list_empty(head)`
///
/// # Safety
///
/// `head` must be a valid pointer to a properly initialized list
#[no_mangle]
pub unsafe extern "C" fn list_empty(head: *const list_head) -> bool {
    if head.is_null() {
        return true;
    }

    (*head).next == (head as *mut list_head)
}

/// Replace old entry with new one
///
/// Linux equivalent: `list_replace(old, new)`
///
/// # Safety
///
/// - Both pointers must be valid
/// - `old` must be in a list
/// - `new` must not be in a list
#[no_mangle]
pub unsafe extern "C" fn list_replace(old: *mut list_head, new: *mut list_head) {
    if old.is_null() || new.is_null() {
        return;
    }

    (*new).next = (*old).next;
    (*(*new).next).prev = new;
    (*new).prev = (*old).prev;
    (*(*new).prev).next = new;
}

/// Move an entry to the head of the list
///
/// Linux equivalent: `list_move(list, head)`
///
/// # Safety
///
/// Both pointers must be valid, `list` must be in a list
#[no_mangle]
pub unsafe extern "C" fn list_move(list: *mut list_head, head: *mut list_head) {
    if list.is_null() || head.is_null() {
        return;
    }

    __list_del((*list).prev, (*list).next);
    list_add(list, head);
}

/// Move an entry to the tail of the list
///
/// Linux equivalent: `list_move_tail(list, head)`
///
/// # Safety
///
/// Both pointers must be valid, `list` must be in a list
#[no_mangle]
pub unsafe extern "C" fn list_move_tail(list: *mut list_head, head: *mut list_head) {
    if list.is_null() || head.is_null() {
        return;
    }

    __list_del((*list).prev, (*list).next);
    list_add_tail(list, head);
}

// Internal helper functions

/// Internal: Add entry between prev and next
///
/// # Safety
///
/// All pointers must be valid
#[inline]
unsafe fn __list_add(new: *mut list_head, prev: *mut list_head, next: *mut list_head) {
    (*next).prev = new;
    (*new).next = next;
    (*new).prev = prev;
    (*prev).next = new;
}

/// Internal: Delete entry by connecting prev and next
///
/// # Safety
///
/// Both pointers must be valid
#[inline]
unsafe fn __list_del(prev: *mut list_head, next: *mut list_head) {
    (*next).prev = prev;
    (*prev).next = next;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_list_head() {
        let mut head = list_head {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        };

        unsafe {
            INIT_LIST_HEAD(&mut head);
            let head_ptr = &mut head as *mut _;
            assert_eq!(head.next, head_ptr);
            assert_eq!(head.prev, head_ptr);
            assert!(list_empty(&head));
        }
    }

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
            assert_eq!(entry.prev, &mut head as *mut _);
        }
    }

    #[test]
    fn test_list_del() {
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
            list_del(&mut entry);

            assert!(list_empty(&head));
            assert!(entry.next.is_null());
            assert!(entry.prev.is_null());
        }
    }
}
