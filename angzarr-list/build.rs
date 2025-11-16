// SPDX-License-Identifier: GPL-2.0
//
// Build script to compile C reference implementation for test verification
//
// This implements Decision #9 from LINUX_KERNEL_LESSONS.md:
// "Direct C Reference Values in Rust Tests"
//
// Purpose:
// - Compile standalone C reference code into Rust test binary
// - Allow Rust tests to call C functions directly
// - Allow Rust tests to access C variables/constants
// - Verify Rust implementation matches C behavior exactly
//
// Unlike trying to compile full Linux kernel code, we use standalone
// C implementations inspired by Linux that compile in userspace.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // Path to standalone C reference implementation
    let c_ref_path = PathBuf::from("../tests/c-reference/list");

    // Check if C reference code exists
    let list_ref_c = c_ref_path.join("list_reference.c");
    if !list_ref_c.exists() {
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("ℹ️  C reference not found at {}", list_ref_c.display());
        eprintln!("   Tests will run without C reference validation.");
        eprintln!("   This is normal during initial development.");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        return;
    }

    println!("cargo:rerun-if-changed={}", list_ref_c.display());

    // Compile standalone C reference implementation
    //
    // This compiles our userspace-compatible C code that mimics Linux behavior
    // without requiring kernel headers.
    cc::Build::new()
        .file(&list_ref_c)
        .include(&c_ref_path)
        .warnings(true)
        .extra_warnings(true)
        .flag("-Werror")
        .flag("-std=c11")
        .opt_level(2)
        .pic(true)
        .compile("list_c_reference");

    // Enable c_reference feature for tests
    println!("cargo:rustc-cfg=c_reference");

    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("✅ Compiled C reference implementation");
    eprintln!("   - Source: {}", list_ref_c.display());
    eprintln!("   - Library: liblist_c_reference.a");
    eprintln!("   - Tests can now validate against C reference");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
