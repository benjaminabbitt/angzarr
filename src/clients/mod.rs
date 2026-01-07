//! External service clients.

pub mod placeholder;
pub mod static_client;

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "go-ffi")]
pub mod go_ffi;

pub use placeholder::PlaceholderBusinessLogic;
pub use static_client::StaticBusinessLogicClient;

#[cfg(feature = "python")]
pub use python::{PyBusinessLogic, PyBusinessLogicBuilder};

#[cfg(feature = "go-ffi")]
pub use go_ffi::{GoBusinessLogic, GoBusinessLogicBuilder};
