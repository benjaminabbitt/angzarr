//! Edition extension trait and constructors.
//!
//! Provides convenience methods for checking timeline status and accessing
//! divergence information.

use crate::proto::Edition;

use super::constants::DEFAULT_EDITION;

/// Extension trait for Edition proto type.
///
/// Provides convenience methods for checking timeline status and accessing
/// divergence information. Constructors remain as associated functions on Edition.
pub trait EditionExt {
    /// Get reference to the edition.
    fn edition_inner(&self) -> &Edition;

    /// Check if this edition has an empty name.
    fn is_empty(&self) -> bool {
        self.edition_inner().name.is_empty()
    }

    /// Check if this is the main timeline (empty or default edition name).
    fn is_main_timeline(&self) -> bool {
        let name = &self.edition_inner().name;
        name.is_empty() || name == DEFAULT_EDITION
    }

    /// Get the edition name, returning the default edition name if empty.
    fn name_or_default(&self) -> &str {
        let edition = self.edition_inner();
        if edition.name.is_empty() {
            DEFAULT_EDITION
        } else {
            &edition.name
        }
    }

    /// Get explicit divergence for a specific domain, if any.
    fn divergence_for(&self, domain: &str) -> Option<u32> {
        self.edition_inner()
            .divergences
            .iter()
            .find(|d| d.domain == domain)
            .map(|d| d.sequence)
    }
}

impl EditionExt for Edition {
    fn edition_inner(&self) -> &Edition {
        self
    }
}

/// Constructors for Edition (cannot be in trait).
impl Edition {
    /// Create an Edition for the main timeline (empty name).
    pub fn main_timeline() -> Self {
        Self {
            name: String::new(),
            divergences: vec![],
        }
    }

    /// Create an Edition with implicit divergence (name only, no explicit divergences).
    pub fn implicit(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            divergences: vec![],
        }
    }

    /// Create an Edition with explicit divergence points.
    pub fn explicit(
        name: impl Into<String>,
        divergences: Vec<crate::proto::DomainDivergence>,
    ) -> Self {
        Self {
            name: name.into(),
            divergences,
        }
    }
}

impl From<&str> for Edition {
    fn from(name: &str) -> Self {
        Edition::implicit(name)
    }
}

impl From<String> for Edition {
    fn from(name: String) -> Self {
        Edition::implicit(name)
    }
}
