//! Handler instrumentation wrappers.
//!
//! Wraps handler traits to emit metrics on handler operations.
//! When the `otel` feature is disabled, passes through with no overhead.

use std::time::Instant;

use async_trait::async_trait;
use tonic::Status;

use crate::orchestration::projector::{ProjectionMode, ProjectorHandler};
use crate::proto::{Cover, EventBook, Notification, Projection, RevocationResponse, SagaResponse};
use crate::standalone::{ProcessManagerHandleResult, ProcessManagerHandler, SagaHandler};

#[cfg(feature = "otel")]
use super::metrics::{
    component_attr, domain_attr, name_attr, outcome_attr, PM_DURATION, PROJECTOR_DURATION,
    SAGA_DURATION,
};

// ============================================================================
// Projector Handler Wrapper
// ============================================================================

/// Wrapper that adds metrics instrumentation to a [`ProjectorHandler`].
///
/// Emits:
/// - `angzarr.projector.duration` - histogram of handler latencies
pub struct InstrumentedProjectorHandler<T> {
    inner: T,
    name: String,
}

impl<T> InstrumentedProjectorHandler<T> {
    /// Wrap a projector handler with metrics instrumentation.
    pub fn new(inner: T, name: impl Into<String>) -> Self {
        Self {
            inner,
            name: name.into(),
        }
    }

    /// Get a reference to the inner handler.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get the projector name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl<T: ProjectorHandler> ProjectorHandler for InstrumentedProjectorHandler<T> {
    async fn handle(&self, events: &EventBook, mode: ProjectionMode) -> Result<Projection, Status> {
        let start = Instant::now();
        let domain = events
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let result = self.inner.handle(events, mode).await;

        #[cfg(feature = "otel")]
        {
            let outcome = if result.is_ok() { "success" } else { "failure" };
            PROJECTOR_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    component_attr("projector"),
                    name_attr(&self.name),
                    domain_attr(domain),
                    outcome_attr(outcome),
                ],
            );
        }
        let _ = (start, domain); // Suppress unused warnings when otel disabled

        result
    }
}

// ============================================================================
// Saga Handler Wrapper
// ============================================================================

/// Wrapper that adds metrics instrumentation to a [`SagaHandler`].
///
/// Emits:
/// - `angzarr.saga.duration` - histogram of handler latencies (for execute phase)
pub struct InstrumentedSagaHandler<T> {
    inner: T,
    name: String,
}

impl<T> InstrumentedSagaHandler<T> {
    /// Wrap a saga handler with metrics instrumentation.
    pub fn new(inner: T, name: impl Into<String>) -> Self {
        Self {
            inner,
            name: name.into(),
        }
    }

    /// Get a reference to the inner handler.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get the saga name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl<T: SagaHandler> SagaHandler for InstrumentedSagaHandler<T> {
    async fn prepare(&self, source: &EventBook) -> Result<Vec<Cover>, Status> {
        // Prepare is typically fast, no metrics needed
        self.inner.prepare(source).await
    }

    async fn execute(
        &self,
        source: &EventBook,
        destinations: &[EventBook],
    ) -> Result<SagaResponse, Status> {
        let start = Instant::now();
        let domain = source
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let result = self.inner.execute(source, destinations).await;

        #[cfg(feature = "otel")]
        {
            let outcome = if result.is_ok() { "success" } else { "failure" };
            SAGA_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    component_attr("saga"),
                    name_attr(&self.name),
                    domain_attr(domain),
                    outcome_attr(outcome),
                ],
            );
        }
        let _ = (start, domain);

        result
    }
}

// ============================================================================
// Process Manager Handler Wrapper
// ============================================================================

/// Wrapper that adds metrics instrumentation to a [`ProcessManagerHandler`].
///
/// Emits:
/// - `angzarr.pm.duration` - histogram of handler latencies
pub struct InstrumentedPMHandler<T> {
    inner: T,
    name: String,
}

impl<T> InstrumentedPMHandler<T> {
    /// Wrap a process manager handler with metrics instrumentation.
    pub fn new(inner: T, name: impl Into<String>) -> Self {
        Self {
            inner,
            name: name.into(),
        }
    }

    /// Get a reference to the inner handler.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get the process manager name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T: ProcessManagerHandler> ProcessManagerHandler for InstrumentedPMHandler<T> {
    fn prepare(&self, trigger: &EventBook, process_state: Option<&EventBook>) -> Vec<Cover> {
        // Prepare is typically fast, no metrics needed
        self.inner.prepare(trigger, process_state)
    }

    fn handle(
        &self,
        trigger: &EventBook,
        process_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> ProcessManagerHandleResult {
        let start = Instant::now();
        let domain = trigger
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("unknown");

        let result = self.inner.handle(trigger, process_state, destinations);

        #[cfg(feature = "otel")]
        {
            PM_DURATION.record(
                start.elapsed().as_secs_f64(),
                &[
                    component_attr("process_manager"),
                    name_attr(&self.name),
                    domain_attr(domain),
                ],
            );
        }
        let _ = (start, domain);

        result
    }

    fn handle_revocation(
        &self,
        notification: &Notification,
        process_state: Option<&EventBook>,
    ) -> (Option<EventBook>, RevocationResponse) {
        self.inner.handle_revocation(notification, process_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProjectorHandler;

    #[async_trait]
    impl ProjectorHandler for MockProjectorHandler {
        async fn handle(
            &self,
            _events: &EventBook,
            _mode: ProjectionMode,
        ) -> Result<Projection, Status> {
            Ok(Projection::default())
        }
    }

    #[tokio::test]
    async fn test_instrumented_projector_delegates() {
        let inner = MockProjectorHandler;
        let handler = InstrumentedProjectorHandler::new(inner, "test-projector");

        let events = EventBook::default();
        let result = handler.handle(&events, ProjectionMode::Execute).await;
        assert!(result.is_ok());
    }

    struct MockSagaHandler;

    #[async_trait]
    impl SagaHandler for MockSagaHandler {
        async fn prepare(&self, _source: &EventBook) -> Result<Vec<Cover>, Status> {
            Ok(vec![])
        }

        async fn execute(
            &self,
            _source: &EventBook,
            _destinations: &[EventBook],
        ) -> Result<SagaResponse, Status> {
            Ok(SagaResponse::default())
        }
    }

    #[tokio::test]
    async fn test_instrumented_saga_delegates() {
        let inner = MockSagaHandler;
        let handler = InstrumentedSagaHandler::new(inner, "test-saga");

        let source = EventBook::default();
        let result = handler.execute(&source, &[]).await;
        assert!(result.is_ok());
    }

    struct MockPMHandler;

    impl ProcessManagerHandler for MockPMHandler {
        fn prepare(&self, _trigger: &EventBook, _process_state: Option<&EventBook>) -> Vec<Cover> {
            vec![]
        }

        fn handle(
            &self,
            _trigger: &EventBook,
            _process_state: Option<&EventBook>,
            _destinations: &[EventBook],
        ) -> ProcessManagerHandleResult {
            ProcessManagerHandleResult::default()
        }
    }

    #[test]
    fn test_instrumented_pm_delegates() {
        let inner = MockPMHandler;
        let handler = InstrumentedPMHandler::new(inner, "test-pm");

        let trigger = EventBook::default();
        let result = handler.handle(&trigger, None, &[]);
        assert!(result.commands.is_empty());
        assert!(result.process_events.is_none());
        assert!(result.facts.is_empty());
    }
}
