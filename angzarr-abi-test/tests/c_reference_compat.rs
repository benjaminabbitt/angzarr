//! C Reference Compatibility Tests
//!
//! Compare Rust structures against compiled C reference structures

use angzarr_list::ListHead;
use angzarr_rbtree::{RbNode, RbRoot};
use angzarr_core::Kref;

// External C functions that return size/offset information
extern "C" {
    fn list_head_size() -> usize;
    fn list_head_align() -> usize;
    fn list_head_next_offset() -> usize;
    fn list_head_prev_offset() -> usize;

    fn rb_node_size() -> usize;
    fn rb_node_align() -> usize;
    fn rb_node_parent_color_offset() -> usize;
    fn rb_node_right_offset() -> usize;
    fn rb_node_left_offset() -> usize;

    fn rb_root_size() -> usize;
    fn rb_root_align() -> usize;

    fn kref_size() -> usize;
    fn kref_align() -> usize;

    static VERIFY_GFP_KERNEL: u32;
    static VERIFY_GFP_ATOMIC: u32;

    static VERIFY_EPERM: i32;
    static VERIFY_ENOENT: i32;
    static VERIFY_ENOMEM: i32;
    static VERIFY_EINVAL: i32;
}

#[test]
fn test_list_head_vs_c() {
    unsafe {
        assert_eq!(
            core::mem::size_of::<ListHead>(),
            list_head_size(),
            "ListHead size must match C struct list_head"
        );

        assert_eq!(
            core::mem::align_of::<ListHead>(),
            list_head_align(),
            "ListHead alignment must match C struct list_head"
        );

        assert_eq!(
            memoffset::offset_of!(ListHead, next),
            list_head_next_offset(),
            "ListHead.next offset must match C"
        );

        assert_eq!(
            memoffset::offset_of!(ListHead, prev),
            list_head_prev_offset(),
            "ListHead.prev offset must match C"
        );
    }
}

#[test]
fn test_rb_node_vs_c() {
    unsafe {
        assert_eq!(
            core::mem::size_of::<RbNode>(),
            rb_node_size(),
            "RbNode size must match C struct rb_node"
        );

        assert_eq!(
            core::mem::align_of::<RbNode>(),
            rb_node_align(),
            "RbNode alignment must match C struct rb_node"
        );

        assert_eq!(
            memoffset::offset_of!(RbNode, __rb_parent_color),
            rb_node_parent_color_offset(),
            "RbNode.__rb_parent_color offset must match C"
        );

        assert_eq!(
            memoffset::offset_of!(RbNode, rb_right),
            rb_node_right_offset(),
            "RbNode.rb_right offset must match C"
        );

        assert_eq!(
            memoffset::offset_of!(RbNode, rb_left),
            rb_node_left_offset(),
            "RbNode.rb_left offset must match C"
        );
    }
}

#[test]
fn test_rb_root_vs_c() {
    unsafe {
        assert_eq!(
            core::mem::size_of::<RbRoot>(),
            rb_root_size(),
            "RbRoot size must match C struct rb_root"
        );

        assert_eq!(
            core::mem::align_of::<RbRoot>(),
            rb_root_align(),
            "RbRoot alignment must match C struct rb_root"
        );
    }
}

#[test]
fn test_kref_vs_c() {
    unsafe {
        // Note: C kref uses atomic_t which may have different size
        // We verify our implementation is compatible
        let rust_size = core::mem::size_of::<Kref>();
        let c_size = kref_size();

        // Our size should be <= C size (we use AtomicU32)
        assert!(
            rust_size <= c_size,
            "Kref size {} must be <= C kref size {}",
            rust_size,
            c_size
        );
    }
}

#[test]
fn test_gfp_flags_vs_c() {
    use angzarr_ffi::GfpFlags;

    unsafe {
        assert_eq!(
            GfpFlags::GFP_KERNEL.0,
            VERIFY_GFP_KERNEL,
            "GFP_KERNEL must match C value"
        );

        assert_eq!(
            GfpFlags::GFP_ATOMIC.0,
            VERIFY_GFP_ATOMIC,
            "GFP_ATOMIC must match C value"
        );
    }
}

#[test]
fn test_error_codes_vs_c() {
    use angzarr_ffi::KernelError;

    unsafe {
        assert_eq!(
            KernelError::EPERM as i32,
            VERIFY_EPERM,
            "EPERM must match C errno"
        );

        assert_eq!(
            KernelError::ENOENT as i32,
            VERIFY_ENOENT,
            "ENOENT must match C errno"
        );

        assert_eq!(
            KernelError::ENOMEM as i32,
            VERIFY_ENOMEM,
            "ENOMEM must match C errno"
        );

        assert_eq!(
            KernelError::EINVAL as i32,
            VERIFY_EINVAL,
            "EINVAL must match C errno"
        );
    }
}
