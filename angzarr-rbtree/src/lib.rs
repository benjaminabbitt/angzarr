//! Linux kernel red-black tree implementation
//!
//! This module provides a Rust implementation of the Linux kernel's red-black tree,
//! maintaining binary compatibility with C code.

#![cfg_attr(not(test), no_std)]

use core::cmp::Ordering;

/// Red-Black tree node color
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RbColor {
    Red = 0,
    Black = 1,
}

/// Red-Black tree node
///
/// This is the Rust equivalent of Linux's `struct rb_node`.
#[repr(C)]
#[derive(Debug)]
pub struct RbNode {
    /// Parent node pointer with color encoded in lowest bit
    pub __rb_parent_color: usize,
    pub rb_right: *mut RbNode,
    pub rb_left: *mut RbNode,
}

unsafe impl Send for RbNode {}
unsafe impl Sync for RbNode {}

impl RbNode {
    /// Create a new uninitialized node
    pub const fn new() -> Self {
        Self {
            __rb_parent_color: 0,
            rb_right: core::ptr::null_mut(),
            rb_left: core::ptr::null_mut(),
        }
    }

    /// Get the parent pointer
    pub fn parent(&self) -> *mut RbNode {
        (self.__rb_parent_color & !1) as *mut RbNode
    }

    /// Get the color of this node
    pub fn color(&self) -> RbColor {
        if (self.__rb_parent_color & 1) == 0 {
            RbColor::Red
        } else {
            RbColor::Black
        }
    }

    /// Set the parent pointer
    ///
    /// # Safety
    /// Caller must ensure parent is valid or null
    pub unsafe fn set_parent(&mut self, parent: *mut RbNode) {
        let color = self.__rb_parent_color & 1;
        self.__rb_parent_color = (parent as usize) | color;
    }

    /// Set the color
    pub fn set_color(&mut self, color: RbColor) {
        let parent = self.__rb_parent_color & !1;
        self.__rb_parent_color = parent | (color as usize);
    }

    /// Check if node is red
    pub fn is_red(&self) -> bool {
        self.color() == RbColor::Red
    }

    /// Check if node is black
    pub fn is_black(&self) -> bool {
        self.color() == RbColor::Black
    }
}

/// Red-Black tree root
#[repr(C)]
#[derive(Debug)]
pub struct RbRoot {
    pub rb_node: *mut RbNode,
}

impl RbRoot {
    /// Create a new empty tree
    pub const fn new() -> Self {
        Self {
            rb_node: core::ptr::null_mut(),
        }
    }

    /// Check if tree is empty
    pub fn is_empty(&self) -> bool {
        self.rb_node.is_null()
    }

    /// Insert a node into the tree
    ///
    /// # Safety
    /// Caller must ensure proper comparison and node validity
    pub unsafe fn insert<F>(&mut self, node: *mut RbNode, compare: F)
    where
        F: Fn(*const RbNode, *const RbNode) -> Ordering,
    {
        let mut parent = core::ptr::null_mut();
        let mut link = &mut self.rb_node as *mut *mut RbNode;

        // Find insertion point
        while !(*link).is_null() {
            parent = *link;
            match compare(node, *link) {
                Ordering::Less => link = &mut (**link).rb_left,
                _ => link = &mut (**link).rb_right,
            }
        }

        // Insert node
        (*node).__rb_parent_color = parent as usize;
        (*node).rb_left = core::ptr::null_mut();
        (*node).rb_right = core::ptr::null_mut();
        (*node).set_color(RbColor::Red);
        *link = node;

        // Rebalance
        self.insert_color(node);
    }

    /// Rebalance tree after insertion
    ///
    /// # Safety
    /// Internal use only
    unsafe fn insert_color(&mut self, mut node: *mut RbNode) {
        while !(*node).parent().is_null() && (*(*node).parent()).is_red() {
            let parent = (*node).parent();
            let grandparent = (*parent).parent();

            if parent == (*grandparent).rb_left {
                let uncle = (*grandparent).rb_right;

                if !uncle.is_null() && (*uncle).is_red() {
                    (*parent).set_color(RbColor::Black);
                    (*uncle).set_color(RbColor::Black);
                    (*grandparent).set_color(RbColor::Red);
                    node = grandparent;
                } else {
                    if node == (*parent).rb_right {
                        node = parent;
                        self.rotate_left(node);
                    }
                    let parent = (*node).parent();
                    let grandparent = (*parent).parent();
                    (*parent).set_color(RbColor::Black);
                    (*grandparent).set_color(RbColor::Red);
                    self.rotate_right(grandparent);
                }
            } else {
                let uncle = (*grandparent).rb_left;

                if !uncle.is_null() && (*uncle).is_red() {
                    (*parent).set_color(RbColor::Black);
                    (*uncle).set_color(RbColor::Black);
                    (*grandparent).set_color(RbColor::Red);
                    node = grandparent;
                } else {
                    if node == (*parent).rb_left {
                        node = parent;
                        self.rotate_right(node);
                    }
                    let parent = (*node).parent();
                    let grandparent = (*parent).parent();
                    (*parent).set_color(RbColor::Black);
                    (*grandparent).set_color(RbColor::Red);
                    self.rotate_left(grandparent);
                }
            }
        }

        if !self.rb_node.is_null() {
            (*self.rb_node).set_color(RbColor::Black);
        }
    }

    /// Rotate left
    ///
    /// # Safety
    /// Internal use only
    unsafe fn rotate_left(&mut self, node: *mut RbNode) {
        let right = (*node).rb_right;
        (*node).rb_right = (*right).rb_left;

        if !(*right).rb_left.is_null() {
            (*(*right).rb_left).set_parent(node);
        }

        (*right).set_parent((*node).parent());

        if (*node).parent().is_null() {
            self.rb_node = right;
        } else if node == (*(*node).parent()).rb_left {
            (*(*node).parent()).rb_left = right;
        } else {
            (*(*node).parent()).rb_right = right;
        }

        (*right).rb_left = node;
        (*node).set_parent(right);
    }

    /// Rotate right
    ///
    /// # Safety
    /// Internal use only
    unsafe fn rotate_right(&mut self, node: *mut RbNode) {
        let left = (*node).rb_left;
        (*node).rb_left = (*left).rb_right;

        if !(*left).rb_right.is_null() {
            (*(*left).rb_right).set_parent(node);
        }

        (*left).set_parent((*node).parent());

        if (*node).parent().is_null() {
            self.rb_node = left;
        } else if node == (*(*node).parent()).rb_right {
            (*(*node).parent()).rb_right = left;
        } else {
            (*(*node).parent()).rb_left = left;
        }

        (*left).rb_right = node;
        (*node).set_parent(left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rb_node_color() {
        let mut node = RbNode::new();
        assert_eq!(node.color(), RbColor::Red);

        node.set_color(RbColor::Black);
        assert_eq!(node.color(), RbColor::Black);
        assert!(node.is_black());

        node.set_color(RbColor::Red);
        assert_eq!(node.color(), RbColor::Red);
        assert!(node.is_red());
    }

    #[test]
    fn test_rb_root_empty() {
        let root = RbRoot::new();
        assert!(root.is_empty());
    }

    #[test]
    fn test_rb_node_parent() {
        let mut node = RbNode::new();
        let mut parent = RbNode::new();

        unsafe {
            node.set_parent(&mut parent as *mut RbNode);
            assert_eq!(node.parent(), &mut parent as *mut RbNode);
        }
    }
}
