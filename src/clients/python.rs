//! Python business logic client via PyO3.
//!
//! Embeds Python interpreter to call business logic handlers.

use std::path::Path;

use async_trait::async_trait;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use tokio::task;
use tracing::{debug, error};

use crate::interfaces::business_client::{BusinessError, BusinessLogicClient, Result};
use crate::proto::{ContextualCommand, EventBook};

/// Python-based business logic client.
///
/// Loads a Python module and calls its `handle` function for command processing.
///
/// # Python Interface
///
/// The Python module must provide a `handle` function:
///
/// ```python
/// def handle(domain: str, command_bytes: bytes) -> bytes:
///     """
///     Handle a contextual command.
///
///     Args:
///         domain: The aggregate domain (e.g., "orders")
///         command_bytes: Serialized ContextualCommand (protobuf)
///
///     Returns:
///         Serialized EventBook (protobuf)
///
///     Raises:
///         Exception: On business logic errors
///     """
///     from evented_pb2 import ContextualCommand, EventBook
///     cmd = ContextualCommand.FromString(command_bytes)
///     # ... process command ...
///     return event_book.SerializeToString()
/// ```
pub struct PyBusinessLogic {
    module_name: String,
    module_path: Option<String>,
    domains: Vec<String>,
}

impl PyBusinessLogic {
    /// Create a new Python business logic client.
    ///
    /// # Arguments
    /// * `module_name` - Python module name (e.g., "my_business_logic")
    /// * `domains` - Domains this handler supports
    pub fn new(module_name: impl Into<String>, domains: Vec<String>) -> Self {
        Self {
            module_name: module_name.into(),
            module_path: None,
            domains,
        }
    }

    /// Create with explicit module path.
    ///
    /// Adds the path to Python's sys.path before importing.
    pub fn with_path(
        module_name: impl Into<String>,
        module_path: impl Into<String>,
        domains: Vec<String>,
    ) -> Self {
        Self {
            module_name: module_name.into(),
            module_path: Some(module_path.into()),
            domains,
        }
    }

    /// Initialize Python and import the module.
    fn call_python(&self, domain: &str, cmd_bytes: Vec<u8>) -> Result<Vec<u8>> {
        Python::with_gil(|py| {
            // Add module path to sys.path if specified
            if let Some(ref path) = self.module_path {
                let sys = py.import_bound("sys")?;
                let sys_path = sys.getattr("path")?;
                if !sys_path.contains(path)? {
                    sys_path.call_method1("insert", (0, path))?;
                }
            }

            // Import the module
            let module = py.import_bound(self.module_name.as_str())?;

            // Call handle function
            let cmd_bytes_py = PyBytes::new_bound(py, &cmd_bytes);
            let result = module.call_method1("handle", (domain, cmd_bytes_py))?;

            // Extract bytes from result
            let result_bytes: &[u8] = result.extract()?;
            Ok(result_bytes.to_vec())
        })
        .map_err(|e: PyErr| {
            error!(error = %e, "Python business logic error");
            BusinessError::Rejected(e.to_string())
        })
    }
}

#[async_trait]
impl BusinessLogicClient for PyBusinessLogic {
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<EventBook> {
        // Serialize command to bytes
        let cmd_bytes = prost::Message::encode_to_vec(&cmd);

        // Clone values for the blocking task
        let domain_owned = domain.to_string();
        let module_name = self.module_name.clone();
        let module_path = self.module_path.clone();
        let domains = self.domains.clone();

        // Run Python in a blocking task to avoid blocking the async runtime
        let result_bytes = task::spawn_blocking(move || {
            let client = PyBusinessLogic {
                module_name,
                module_path,
                domains,
            };
            client.call_python(&domain_owned, cmd_bytes)
        })
        .await
        .map_err(|e| BusinessError::Rejected(format!("Task join error: {}", e)))??;

        // Deserialize result
        let event_book: EventBook = prost::Message::decode(result_bytes.as_slice())
            .map_err(|e| BusinessError::Rejected(format!("Failed to decode EventBook: {}", e)))?;

        debug!(
            domain = %domain,
            event_count = event_book.pages.len(),
            "Python handler returned events"
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

/// Builder for creating Python business logic clients.
pub struct PyBusinessLogicBuilder {
    module_name: String,
    module_path: Option<String>,
    domains: Vec<String>,
}

impl PyBusinessLogicBuilder {
    /// Start building a Python business logic client.
    pub fn new(module_name: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            module_path: None,
            domains: Vec::new(),
        }
    }

    /// Set the path where the Python module is located.
    pub fn module_path(mut self, path: impl AsRef<Path>) -> Self {
        self.module_path = Some(path.as_ref().to_string_lossy().into_owned());
        self
    }

    /// Add a domain this handler supports.
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domains.push(domain.into());
        self
    }

    /// Add multiple domains this handler supports.
    pub fn domains(mut self, domains: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.domains.extend(domains.into_iter().map(|d| d.into()));
        self
    }

    /// Build the Python business logic client.
    pub fn build(self) -> PyBusinessLogic {
        PyBusinessLogic {
            module_name: self.module_name,
            module_path: self.module_path,
            domains: self.domains,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creates_client() {
        let client = PyBusinessLogicBuilder::new("my_module")
            .module_path("/path/to/module")
            .domain("orders")
            .domain("inventory")
            .build();

        assert_eq!(client.module_name, "my_module");
        assert_eq!(client.module_path, Some("/path/to/module".to_string()));
        assert_eq!(client.domains, vec!["orders", "inventory"]);
    }

    #[test]
    fn test_has_domain_empty_matches_all() {
        let client = PyBusinessLogic::new("test", vec![]);
        assert!(client.has_domain("anything"));
    }

    #[test]
    fn test_has_domain_specific() {
        let client = PyBusinessLogic::new("test", vec!["orders".to_string()]);
        assert!(client.has_domain("orders"));
        assert!(!client.has_domain("inventory"));
    }
}
