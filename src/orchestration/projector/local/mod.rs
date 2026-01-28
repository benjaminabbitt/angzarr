//! Local (in-process) projector context.
//!
//! Delegates event handling directly to in-process `ProjectorHandler`.

use std::sync::Arc;

use async_trait::async_trait;

use crate::proto::{EventBook, Projection};
use crate::standalone::ProjectorHandler;

use super::ProjectorContext;

/// In-process projector context that calls the handler directly.
pub struct LocalProjectorContext {
    handler: Arc<dyn ProjectorHandler>,
}

impl LocalProjectorContext {
    /// Create with an in-process projector handler.
    pub fn new(handler: Arc<dyn ProjectorHandler>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl ProjectorContext for LocalProjectorContext {
    async fn handle_events(&self, events: &EventBook) -> Result<Projection, tonic::Status> {
        self.handler.handle(events).await
    }
}
