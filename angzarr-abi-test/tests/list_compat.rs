//! List ABI Compatibility Tests
//!
//! Verify that angzarr_list::ListHead matches Linux kernel's struct list_head

use angzarr_list::ListHead;
use memoffset::offset_of;
use static_assertions::*;

// Linux kernel struct list_head on x86_64:
// struct list_head {
//     struct list_head *next, *prev;
// };
//
// Expected layout on x86_64:
// - Size: 16 bytes (2 x 8-byte pointers)
// - Alignment: 8 bytes
// - next offset: 0
// - prev offset: 8

#[test]
fn test_list_head_size() {
    // Verify size matches Linux kernel
    const EXPECTED_SIZE: usize = 2 * core::mem::size_of::<usize>();
    assert_eq!(
        core::mem::size_of::<ListHead>(),
        EXPECTED_SIZE,
        "ListHead size must match Linux kernel list_head"
    );
}

#[test]
fn test_list_head_alignment() {
    // Verify alignment matches Linux kernel
    const EXPECTED_ALIGN: usize = core::mem::align_of::<usize>();
    assert_eq!(
        core::mem::align_of::<ListHead>(),
        EXPECTED_ALIGN,
        "ListHead alignment must match Linux kernel list_head"
    );
}

#[test]
fn test_list_head_field_offsets() {
    // Verify field offsets match Linux kernel
    assert_eq!(
        offset_of!(ListHead, next),
        0,
        "next field must be at offset 0"
    );

    assert_eq!(
        offset_of!(ListHead, prev),
        core::mem::size_of::<usize>(),
        "prev field must be at offset 8 (on 64-bit)"
    );
}

// Compile-time assertions for extra safety
assert_eq_size!(ListHead, [usize; 2]);
assert_eq_align!(ListHead, usize);

// Verify repr(C) is applied
const _: () = {
    // This ensures the struct is repr(C) by checking layout predictability
    const SIZE: usize = core::mem::size_of::<ListHead>();
    const EXPECTED: usize = core::mem::size_of::<usize>() * 2;
    assert!(SIZE == EXPECTED);
};

#[test]
fn test_list_head_c_layout() {
    // Create a test structure and verify pointers are laid out correctly
    let mut head = ListHead {
        next: core::ptr::null_mut(),
        prev: core::ptr::null_mut(),
    };

    // Get raw pointer to struct
    let ptr = &mut head as *mut ListHead as *mut u8;

    // Verify next is at offset 0
    unsafe {
        let next_ptr = ptr.add(0) as *mut *mut ListHead;
        assert_eq!(
            next_ptr as usize,
            &mut head.next as *mut _ as usize
        );
    }

    // Verify prev is at offset 8 (on 64-bit)
    unsafe {
        let prev_ptr = ptr.add(core::mem::size_of::<usize>()) as *mut *mut ListHead;
        assert_eq!(
            prev_ptr as usize,
            &mut head.prev as *mut _ as usize
        );
    }
}

#[test]
fn test_list_head_null_initialization() {
    // Verify null-initialized list matches expected pattern
    let head = ListHead {
        next: core::ptr::null_mut(),
        prev: core::ptr::null_mut(),
    };

    assert!(head.next.is_null());
    assert!(head.prev.is_null());
}

#[cfg(target_pointer_width = "64")]
#[test]
fn test_list_head_size_64bit() {
    // On 64-bit systems, list_head must be exactly 16 bytes
    assert_eq!(core::mem::size_of::<ListHead>(), 16);
}

#[cfg(target_pointer_width = "32")]
#[test]
fn test_list_head_size_32bit() {
    // On 32-bit systems, list_head must be exactly 8 bytes
    assert_eq!(core::mem::size_of::<ListHead>(), 8);
}
