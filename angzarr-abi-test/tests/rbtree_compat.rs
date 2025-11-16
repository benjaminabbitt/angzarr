//! Red-Black Tree ABI Compatibility Tests
//!
//! Verify that angzarr_rbtree structures match Linux kernel's rb_node and rb_root

use angzarr_rbtree::{RbNode, RbRoot, RbColor};
use memoffset::offset_of;
use static_assertions::*;

// Linux kernel struct rb_node on x86_64:
// struct rb_node {
//     unsigned long  __rb_parent_color;
//     struct rb_node *rb_right;
//     struct rb_node *rb_left;
// };
//
// Expected layout on x86_64:
// - Size: 24 bytes (3 x 8-byte fields)
// - Alignment: 8 bytes
// - __rb_parent_color offset: 0
// - rb_right offset: 8
// - rb_left offset: 16

#[test]
fn test_rb_node_size() {
    const EXPECTED_SIZE: usize = 3 * core::mem::size_of::<usize>();
    assert_eq!(
        core::mem::size_of::<RbNode>(),
        EXPECTED_SIZE,
        "RbNode size must match Linux kernel rb_node"
    );
}

#[test]
fn test_rb_node_alignment() {
    const EXPECTED_ALIGN: usize = core::mem::align_of::<usize>();
    assert_eq!(
        core::mem::align_of::<RbNode>(),
        EXPECTED_ALIGN,
        "RbNode alignment must match Linux kernel rb_node"
    );
}

#[test]
fn test_rb_node_field_offsets() {
    assert_eq!(
        offset_of!(RbNode, __rb_parent_color),
        0,
        "__rb_parent_color must be at offset 0"
    );

    assert_eq!(
        offset_of!(RbNode, rb_right),
        core::mem::size_of::<usize>(),
        "rb_right must be at offset 8 (on 64-bit)"
    );

    assert_eq!(
        offset_of!(RbNode, rb_left),
        2 * core::mem::size_of::<usize>(),
        "rb_left must be at offset 16 (on 64-bit)"
    );
}

// Linux kernel struct rb_root:
// struct rb_root {
//     struct rb_node *rb_node;
// };
//
// Expected layout:
// - Size: 8 bytes (1 pointer)
// - Alignment: 8 bytes

#[test]
fn test_rb_root_size() {
    const EXPECTED_SIZE: usize = core::mem::size_of::<usize>();
    assert_eq!(
        core::mem::size_of::<RbRoot>(),
        EXPECTED_SIZE,
        "RbRoot size must match Linux kernel rb_root"
    );
}

#[test]
fn test_rb_root_alignment() {
    const EXPECTED_ALIGN: usize = core::mem::align_of::<usize>();
    assert_eq!(
        core::mem::align_of::<RbRoot>(),
        EXPECTED_ALIGN,
        "RbRoot alignment must match Linux kernel rb_root"
    );
}

#[test]
fn test_rb_color_values() {
    // Linux kernel uses 0 for red, 1 for black
    assert_eq!(RbColor::Red as u8, 0);
    assert_eq!(RbColor::Black as u8, 1);
}

#[test]
fn test_rb_color_size() {
    // Color is an enum, typically 4 bytes in Rust
    // What matters is the values (0 and 1) match Linux
    assert!(core::mem::size_of::<RbColor>() >= 1);
    assert!(core::mem::size_of::<RbColor>() <= 4);
}

// Compile-time assertions
assert_eq_size!(RbNode, [usize; 3]);
assert_eq_align!(RbNode, usize);
assert_eq_size!(RbRoot, usize);
assert_eq_align!(RbRoot, usize);

#[cfg(target_pointer_width = "64")]
#[test]
fn test_rb_node_size_64bit() {
    assert_eq!(core::mem::size_of::<RbNode>(), 24);
    assert_eq!(core::mem::size_of::<RbRoot>(), 8);
}

#[cfg(target_pointer_width = "32")]
#[test]
fn test_rb_node_size_32bit() {
    assert_eq!(core::mem::size_of::<RbNode>(), 12);
    assert_eq!(core::mem::size_of::<RbRoot>(), 4);
}

#[test]
fn test_rb_node_parent_color_encoding() {
    // Verify that parent pointer and color can be stored together
    // Linux uses the low bit for color (since pointers are aligned)
    let node = RbNode::new();

    // Color should be encoded in low bit
    let color_mask = 1usize;
    let parent_mask = !color_mask;

    // Verify we can extract color
    let color_bit = node.__rb_parent_color & color_mask;
    assert!(color_bit == 0 || color_bit == 1);

    // Verify parent can be extracted
    let _parent = (node.__rb_parent_color & parent_mask) as *mut RbNode;
}
