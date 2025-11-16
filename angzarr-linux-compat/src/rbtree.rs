//! Linux-compatible red-black tree API adapter
//!
//! Provides Linux rb_tree compatible API.

use angzarr_rbtree::RbColor;

// Re-export Angzarr types as Linux-compatible names
pub use angzarr_rbtree::RbNode as rb_node;
pub use angzarr_rbtree::RbRoot as rb_root;

/// Linux-compatible color values
pub const RB_RED: u32 = 0;
pub const RB_BLACK: u32 = 1;

/// Get color of node
///
/// # Safety
///
/// `node` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn rb_color(node: *const rb_node) -> u32 {
    if node.is_null() {
        return RB_BLACK;
    }

    match (*node).color() {
        RbColor::Red => RB_RED,
        RbColor::Black => RB_BLACK,
    }
}

/// Set color of node
///
/// # Safety
///
/// `node` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn rb_set_red(node: *mut rb_node) {
    if !node.is_null() {
        (*node).set_color(RbColor::Red);
    }
}

/// Set color of node to black
///
/// # Safety
///
/// `node` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn rb_set_black(node: *mut rb_node) {
    if !node.is_null() {
        (*node).set_color(RbColor::Black);
    }
}

/// Get parent of node
///
/// # Safety
///
/// `node` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn rb_parent(node: *const rb_node) -> *mut rb_node {
    if node.is_null() {
        return core::ptr::null_mut();
    }

    (*node).parent()
}

/// Check if tree is empty
///
/// # Safety
///
/// `root` must be a valid pointer
#[no_mangle]
pub unsafe extern "C" fn rb_empty(root: *const rb_root) -> bool {
    if root.is_null() {
        return true;
    }

    (*root).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rb_empty() {
        let root = rb_root::new();
        unsafe {
            assert!(rb_empty(&root));
        }
    }

    #[test]
    fn test_rb_color() {
        let node = rb_node::new();
        unsafe {
            // Initial color is red
            assert_eq!(rb_color(&node), RB_RED);

            let mut node_mut = node;
            rb_set_black(&mut node_mut);
            assert_eq!(rb_color(&node_mut), RB_BLACK);

            rb_set_red(&mut node_mut);
            assert_eq!(rb_color(&node_mut), RB_RED);
        }
    }
}
