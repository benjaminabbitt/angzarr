//! ABI Compatibility Testing for Angzarr
//!
//! This crate provides comprehensive testing to ensure Rust structures
//! are binary-compatible with Linux kernel C structures.

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use core::mem::{size_of, align_of};

/// Macro to verify structure size matches expected value
#[macro_export]
macro_rules! verify_size {
    ($rust_type:ty, $expected_size:expr) => {
        const _: () = {
            const SIZE: usize = ::core::mem::size_of::<$rust_type>();
            const EXPECTED: usize = $expected_size;

            // This will fail at compile time if sizes don't match
            assert!(SIZE == EXPECTED, "Size mismatch");
        };
    };
}

/// Macro to verify field offset matches expected value
#[macro_export]
macro_rules! verify_offset {
    ($type:ty, $field:ident, $expected_offset:expr) => {
        const _: () = {
            use memoffset::offset_of;
            const OFFSET: usize = offset_of!($type, $field);
            const EXPECTED: usize = $expected_offset;

            assert!(OFFSET == EXPECTED, "Offset mismatch");
        };
    };
}

/// Macro to verify type alignment
#[macro_export]
macro_rules! verify_align {
    ($type:ty, $expected_align:expr) => {
        const _: () = {
            const ALIGN: usize = ::core::mem::align_of::<$type>();
            const EXPECTED: usize = $expected_align;

            assert!(ALIGN == EXPECTED, "Alignment mismatch");
        };
    };
}

/// Structure to hold ABI compatibility test results
#[derive(Debug, PartialEq, Eq)]
pub struct AbiCompatResult {
    pub struct_name: &'static str,
    pub size_match: bool,
    pub align_match: bool,
    pub fields_match: bool,
}

impl AbiCompatResult {
    pub fn is_compatible(&self) -> bool {
        self.size_match && self.align_match && self.fields_match
    }
}

/// Test helper to compare Rust and C structure layouts
pub fn verify_struct_layout<T>(
    name: &'static str,
    expected_size: usize,
    expected_align: usize,
) -> AbiCompatResult {
    AbiCompatResult {
        struct_name: name,
        size_match: size_of::<T>() == expected_size,
        align_match: align_of::<T>() == expected_align,
        fields_match: true, // Set by individual field tests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_compat_result() {
        let result = AbiCompatResult {
            struct_name: "test",
            size_match: true,
            align_match: true,
            fields_match: true,
        };
        assert!(result.is_compatible());
    }
}
