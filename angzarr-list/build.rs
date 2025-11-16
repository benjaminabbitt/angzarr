// SPDX-License-Identifier: GPL-2.0
//
// Build script to compile Linux kernel list helpers for test verification
//
// This implements Decision #9 from LINUX_KERNEL_LESSONS.md:
// "Direct C Reference Values in Rust Tests"
//
// Purpose:
// - Compile Linux kernel C code into our Rust test binary
// - Allow Rust tests to call C functions directly
// - Allow Rust tests to access C variables/constants
// - Verify Rust implementation matches C behavior exactly

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // C reference compilation is currently DISABLED by default because
    // Linux kernel code requires architecture-specific headers that
    // aren't available in standard userspace compilation.
    //
    // Future work: Create minimal standalone C reference implementations
    // instead of compiling full kernel code.
    //
    // For now, tests validate against documented Linux behavior without
    // runtime C comparison.

    let _enable_c_ref = std::env::var("ANGZARR_ENABLE_C_REFERENCE")
        .unwrap_or_default()
        == "1";

    // Always return early for now - C compilation not yet working
    eprintln!("ℹ️  Note: C reference compilation is not yet enabled");
    eprintln!("   Tests validate against documented Linux behavior.");
    eprintln!("   See LINUX_TEST_MAPPING.md for test traceability.");
    return;

    // TODO: Uncomment when we have standalone C reference code
    // that doesn't require full kernel headers
    /*
    if !enable_c_ref {
        return;
    }

    let kernel_path = PathBuf::from("../tests/linux-kernel");
    if !kernel_path.exists() {
        panic!("ANGZARR_ENABLE_C_REFERENCE=1 but submodule not found");
    }

    let list_sort = kernel_path.join("lib/list_sort.c");
    println!("cargo:rerun-if-changed={}", list_sort.display());
    println!("cargo:rustc-cfg=feature=\"c_reference\"");

    cc::Build::new()
        .file(&list_sort)
        .include(kernel_path.join("include"))
        .warnings(false)
        .compile("linux_list_reference");
    */
}
