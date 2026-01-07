//! Go FFI business logic client via libloading.
//!
//! Loads a Go shared library (.so) and calls its Handle function.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use async_trait::async_trait;
use libloading::{Library, Symbol};
use tokio::task;
use tracing::{debug, error};

use crate::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use crate::proto::{ContextualCommand, EventBook};

/// Type signature for the Go Handle function.
/// The tuple return is safe in practice - Go's CGo uses a struct for multi-returns.
#[allow(improper_ctypes_definitions)]
type HandleFn = unsafe extern "C" fn(*const c_char, *const c_char, c_int) -> (*mut c_char, c_int);

/// Type signature for the Go FreeResult function.
type FreeResultFn = unsafe extern "C" fn(*mut c_char);

/// Go FFI business logic client.
///
/// Loads a Go shared library and calls its exported Handle function.
///
/// # Go Interface
///
/// The Go library must export these functions:
///
/// ```go
/// //export Handle
/// func Handle(domain *C.char, cmdPtr *C.char, cmdLen C.int) (*C.char, C.int)
///
/// //export FreeResult
/// func FreeResult(ptr *C.char)
/// ```
pub struct GoBusinessLogic {
    library_path: String,
    domains: Vec<String>,
}

impl GoBusinessLogic {
    /// Create a new Go FFI business logic client.
    ///
    /// # Arguments
    /// * `library_path` - Path to the Go shared library (.so file)
    /// * `domains` - Domains this handler supports
    pub fn new(library_path: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            library_path: library_path.into(),
            domains,
        }
    }

    /// Call the Go Handle function.
    fn call_go(&self, domain: &str, cmd_bytes: Vec<u8>) -> Result<Vec<u8>> {
        // Load the library
        let lib = unsafe { Library::new(&self.library_path) }.map_err(|e| {
            error!(error = %e, path = %self.library_path, "Failed to load Go library");
            BusinessError::Rejected(format!("Failed to load library: {}", e))
        })?;

        // Get function pointers
        let handle: Symbol<HandleFn> = unsafe { lib.get(b"Handle") }.map_err(|e| {
            error!(error = %e, "Failed to find Handle function");
            BusinessError::Rejected(format!("Handle function not found: {}", e))
        })?;

        let free_result: Symbol<FreeResultFn> =
            unsafe { lib.get(b"FreeResult") }.map_err(|e| {
                error!(error = %e, "Failed to find FreeResult function");
                BusinessError::Rejected(format!("FreeResult function not found: {}", e))
            })?;

        // Prepare arguments
        let domain_cstr = CString::new(domain).map_err(|e| {
            BusinessError::Rejected(format!("Invalid domain string: {}", e))
        })?;

        // Call Go function
        let (result_ptr, result_len) = unsafe {
            handle(
                domain_cstr.as_ptr(),
                cmd_bytes.as_ptr() as *const c_char,
                cmd_bytes.len() as c_int,
            )
        };

        // Handle result
        if result_len < 0 {
            // Error: result_ptr contains error message
            let error_msg = if !result_ptr.is_null() {
                let msg = unsafe { CStr::from_ptr(result_ptr) }
                    .to_string_lossy()
                    .into_owned();
                unsafe { free_result(result_ptr) };
                msg
            } else {
                "Unknown error from Go handler".to_string()
            };
            return Err(BusinessError::Rejected(error_msg));
        }

        // Success: copy result bytes
        let result_bytes = if !result_ptr.is_null() && result_len > 0 {
            let bytes =
                unsafe { std::slice::from_raw_parts(result_ptr as *const u8, result_len as usize) }
                    .to_vec();
            unsafe { free_result(result_ptr) };
            bytes
        } else {
            vec![]
        };

        Ok(result_bytes)
    }
}

#[async_trait]
impl BusinessLogicClient for GoBusinessLogic {
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<EventBook> {
        // Serialize command to bytes
        let cmd_bytes = prost::Message::encode_to_vec(&cmd);

        // Clone for blocking task
        let domain_owned = domain.to_string();
        let library_path = self.library_path.clone();
        let domains = self.domains.clone();

        // Run in blocking task to avoid blocking async runtime
        let result_bytes = task::spawn_blocking(move || {
            let client = GoBusinessLogic {
                library_path,
                domains,
            };
            client.call_go(&domain_owned, cmd_bytes)
        })
        .await
        .map_err(|e| BusinessError::Rejected(format!("Task join error: {}", e)))??;

        // Deserialize result
        let event_book: EventBook = prost::Message::decode(result_bytes.as_slice())
            .map_err(|e| BusinessError::Rejected(format!("Failed to decode EventBook: {}", e)))?;

        debug!(
            domain = %domain,
            event_count = event_book.pages.len(),
            "Go handler returned events"
        );

        Ok(event_book)
    }

    fn has_domain(&self, domain: &str) -> bool {
        self.domains.is_empty() || self.domains.iter().any(|d| d == domain)
    }

    fn domains(&self) -> Vec<String> {
        self.domains.clone()
    }
}

/// Builder for Go business logic clients.
pub struct GoBusinessLogicBuilder {
    library_path: String,
    domains: Vec<String>,
}

impl GoBusinessLogicBuilder {
    /// Start building a Go business logic client.
    pub fn new(library_path: impl AsRef<Path>) -> Self {
        Self {
            library_path: library_path.as_ref().to_string_lossy().into_owned(),
            domains: Vec::new(),
        }
    }

    /// Add a domain this handler supports.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domains.push(domain.into());
        self
    }

    /// Add multiple domains.
    pub fn domains(mut self, domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.domains.extend(domains.into_iter().map(|d| d.into()));
        self
    }

    /// Build the Go business logic client.
    pub fn build(self) -> GoBusinessLogic {
        GoBusinessLogic {
            library_path: self.library_path,
            domains: self.domains,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creates_client() {
        let client = GoBusinessLogicBuilder::new("/path/to/lib.so")
            .domain("orders")
            .domain("discounts")
            .build();

        assert_eq!(client.library_path, "/path/to/lib.so");
        assert_eq!(client.domains, vec!["orders", "discounts"]);
    }

    #[test]
    fn test_has_domain_empty_matches_all() {
        let client = GoBusinessLogic::new("/lib.so", vec![]);
        assert!(client.has_domain("anything"));
    }

    #[test]
    fn test_has_domain_specific() {
        let client = GoBusinessLogic::new("/lib.so", vec!["orders".to_string()]);
        assert!(client.has_domain("orders"));
        assert!(!client.has_domain("inventory"));
    }
}
