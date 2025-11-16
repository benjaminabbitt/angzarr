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

    // Path to Linux kernel submodule
    let kernel_path = PathBuf::from("../tests/linux-kernel");

    // Check if submodule is initialized
    if !kernel_path.exists() {
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("⚠️  Warning: Linux kernel submodule not initialized");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!();
        eprintln!("Tests will run without C reference validation.");
        eprintln!();
        eprintln!("To enable C reference validation:");
        eprintln!("  cd ..");
        eprintln!("  git submodule update --init --recursive");
        eprintln!();
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Exit gracefully - tests will still run, just without C validation
        return;
    }

    // Check if test files exist
    let list_sort = kernel_path.join("lib/list_sort.c");
    if !list_sort.exists() {
        eprintln!("Warning: {} not found", list_sort.display());
        eprintln!("Kernel submodule may be incomplete");
        return;
    }

    println!("cargo:rerun-if-changed={}", list_sort.display());

    // Enable C reference feature for tests
    println!("cargo:rustc-cfg=feature=\"c_reference\"");

    // Compile Linux kernel list_sort.c for reference in tests
    //
    // Note: We compile list_sort.c because it contains well-tested
    // list manipulation functions that we can use as reference.
    // The test suite will call these C functions and compare results
    // with our Rust implementation.
    cc::Build::new()
        .file(&list_sort)
        .include(kernel_path.join("include"))
        // Disable warnings - Linux kernel code has many warnings
        // when compiled in userspace
        .warnings(false)
        // Define __KERNEL__ to enable kernel-specific code paths
        .define("__KERNEL__", None)
        // Optimization level matches test builds
        .opt_level(2)
        // Static library linked into test binary
        .compile("linux_list_reference");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ Compiled Linux kernel C reference code");
    println!("   - list_sort.c → liblinux_list_reference.a");
    println!("   - Linked into test binary");
    println!("   - Tests can now validate against C reference");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
