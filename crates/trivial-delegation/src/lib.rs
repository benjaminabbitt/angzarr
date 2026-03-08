//! Attribute macro for marking trivial delegation functions.
//!
//! Functions marked with `#[trivial_delegation]` are excluded from:
//! - Mutation testing (`#[mutants::skip]`)
//! - Coverage analysis (`#[coverage(off)]` on nightly with `coverage_nightly` feature)
//!
//! These functions are tested via integration tests, not unit tests.
//!
//! # Example
//!
//! ```ignore
//! #[trivial_delegation]
//! pub fn has_domain(&self, domain: &str) -> bool {
//!     self.router.has_handler(domain)
//! }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Item};

/// Marks a function as trivial delegation that doesn't warrant unit testing.
///
/// Effects:
/// - Always: `#[mutants::skip]` - excluded from mutation testing
/// - With `coverage_nightly` feature on nightly: `#[coverage(off)]` - excluded from coverage
///
/// Use this for single-line delegation methods that just forward to an inner type,
/// possibly with error mapping. These are tested via integration tests.
#[proc_macro_attribute]
pub fn trivial_delegation(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);

    let output = quote! {
        #[mutants::skip]
        #[cfg_attr(coverage_nightly, coverage(off))]
        #input
    };

    output.into()
}
