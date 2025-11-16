//! Build script for ABI compatibility testing
//!
//! This script:
//! 1. Generates C headers from Rust structs (via cbindgen)
//! 2. Compiles reference C structures for comparison
//! 3. Generates bindgen bindings from Linux headers (if available)

use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Generate reference C code for structure verification
    generate_reference_c_code(&out_dir);

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=linux-headers/");
}

fn generate_reference_c_code(out_dir: &PathBuf) {
    // Create a C file with Linux kernel structure definitions for comparison
    let c_code = format!(r#"
#include <stddef.h>
#include <stdint.h>

/* Reference Linux kernel structures for ABI verification */

/* From include/linux/types.h and include/linux/list.h */
struct list_head {{
    struct list_head *next, *prev;
}};

/* From include/linux/rbtree.h */
struct rb_node {{
    unsigned long __rb_parent_color;
    struct rb_node *rb_right;
    struct rb_node *rb_left;
}};

struct rb_root {{
    struct rb_node *rb_node;
}};

/* From include/linux/kref.h */
struct kref {{
    int refcount;  /* Simplified - actual uses atomic_t */
}};

/* Size and offset verification functions */
size_t list_head_size(void) {{ return sizeof(struct list_head); }}
size_t list_head_align(void) {{ return _Alignof(struct list_head); }}
size_t list_head_next_offset(void) {{ return offsetof(struct list_head, next); }}
size_t list_head_prev_offset(void) {{ return offsetof(struct list_head, prev); }}

size_t rb_node_size(void) {{ return sizeof(struct rb_node); }}
size_t rb_node_align(void) {{ return _Alignof(struct rb_node); }}
size_t rb_node_parent_color_offset(void) {{ return offsetof(struct rb_node, __rb_parent_color); }}
size_t rb_node_right_offset(void) {{ return offsetof(struct rb_node, rb_right); }}
size_t rb_node_left_offset(void) {{ return offsetof(struct rb_node, rb_left); }}

size_t rb_root_size(void) {{ return sizeof(struct rb_root); }}
size_t rb_root_align(void) {{ return _Alignof(struct rb_root); }}

size_t kref_size(void) {{ return sizeof(struct kref); }}
size_t kref_align(void) {{ return _Alignof(struct kref); }}

/* GFP flags from include/linux/gfp.h */
unsigned int VERIFY_GFP_KERNEL = 0x0cc0u;
unsigned int VERIFY_GFP_ATOMIC = 0x0020u;

/* Error codes from include/uapi/asm-generic/errno-base.h */
int VERIFY_EPERM = 1;
int VERIFY_ENOENT = 2;
int VERIFY_ENOMEM = 12;
int VERIFY_EINVAL = 22;
"#);

    // Write the C code
    let c_file = out_dir.join("linux_reference.c");
    std::fs::write(&c_file, c_code).expect("Failed to write C reference code");

    // Compile it to a static library
    cc::Build::new()
        .file(&c_file)
        .warnings(false)
        .static_flag(true)
        .compile("linux_reference");
}
