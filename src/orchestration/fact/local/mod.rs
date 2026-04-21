//! Local (in-process) fact executor.
//!
//! Injects facts via a `FactRouterExecutor` trait (direct pipeline calls).

use std::sync::Arc;

use async_trait::async_trait;

use crate::orchestration::FactInjectionError;
use crate::proto::EventBook;

use crate::orchestration::FactExecutor;

/// Trait for injecting facts via a local command router.
///
/// Abstracts the fact injection pipeline so local fact executor doesn't
/// depend on specific router implementation types.
#[async_trait]
pub trait FactRouterExecutor: Send + Sync {
    /// Inject a fact into the target aggregate identified by the fact's cover domain.
    async fn execute_fact(
        &self,
        fact: EventBook,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Injects facts via in-process fact router.
pub struct LocalFactExecutor {
    router: Arc<dyn FactRouterExecutor>,
}

impl LocalFactExecutor {
    /// Create with a reference to a fact router executor.
    pub fn new(router: Arc<dyn FactRouterExecutor>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl FactExecutor for LocalFactExecutor {
    async fn inject(&self, fact: EventBook) -> Result<(), FactInjectionError> {
        self.router
            .execute_fact(fact)
            .await
            .map_err(|e| FactInjectionError::Internal(e.to_string()))
    }
}

#[cfg(test)]
#[path = "mod.test.rs"]
mod tests;
