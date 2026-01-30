//! gRPC process manager context.
//!
//! Delegates prepare/handle to remote `ProcessManagerClient` via gRPC.
//! Persists PM events by routing through `CommandExecutor`.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::orchestration::command::{CommandExecutor, CommandOutcome};
use crate::proto::process_manager_client::ProcessManagerClient;
use crate::proto::{
    CommandBook, Cover, EventBook, ProcessManagerHandleRequest, ProcessManagerPrepareRequest,
};
use crate::proto_ext::{correlated_request, CoverExt};

use super::{PMContextFactory, PmHandleResponse, ProcessManagerContext};

/// gRPC PM context that calls remote ProcessManager service.
pub struct GrpcPMContext {
    client: Arc<Mutex<ProcessManagerClient<tonic::transport::Channel>>>,
    command_executor: Arc<dyn CommandExecutor>,
    pm_domain: String,
}

impl GrpcPMContext {
    /// Create with gRPC client, command executor, and PM domain.
    pub fn new(
        client: Arc<Mutex<ProcessManagerClient<tonic::transport::Channel>>>,
        command_executor: Arc<dyn CommandExecutor>,
        pm_domain: String,
    ) -> Self {
        Self {
            client,
            command_executor,
            pm_domain,
        }
    }
}

#[async_trait]
impl ProcessManagerContext for GrpcPMContext {
    async fn prepare(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
    ) -> Result<Vec<Cover>, Box<dyn std::error::Error + Send + Sync>> {
        let correlation_id = trigger.correlation_id();
        let request = ProcessManagerPrepareRequest {
            trigger: Some(trigger.clone()),
            process_state: pm_state.cloned(),
        };

        let mut client = self.client.lock().await;
        let response = client
            .prepare(correlated_request(request, correlation_id))
            .await?
            .into_inner();
        Ok(response.destinations)
    }

    async fn handle(
        &self,
        trigger: &EventBook,
        pm_state: Option<&EventBook>,
        destinations: &[EventBook],
    ) -> Result<PmHandleResponse, Box<dyn std::error::Error + Send + Sync>> {
        let correlation_id = trigger.correlation_id();
        let request = ProcessManagerHandleRequest {
            trigger: Some(trigger.clone()),
            process_state: pm_state.cloned(),
            destinations: destinations.to_vec(),
        };

        let mut client = self.client.lock().await;
        let response = client
            .handle(correlated_request(request, correlation_id))
            .await?
            .into_inner();

        Ok(PmHandleResponse {
            commands: response.commands,
            process_events: response.process_events,
        })
    }

    async fn persist_pm_events(
        &self,
        process_events: &EventBook,
        correlation_id: &str,
    ) -> CommandOutcome {
        let pm_command = CommandBook {
            cover: Some(Cover {
                domain: self.pm_domain.clone(),
                root: process_events.cover.as_ref().and_then(|c| c.root.clone()),
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages: vec![],
            saga_origin: None,
        };

        self.command_executor.execute(pm_command).await
    }
}

/// Factory that produces `GrpcPMContext` instances for distributed mode.
///
/// Captures long-lived gRPC client and command executor.
/// Each call to `create()` produces a context for one PM invocation.
pub struct GrpcPMContextFactory {
    client: Arc<Mutex<ProcessManagerClient<tonic::transport::Channel>>>,
    command_executor: Arc<dyn CommandExecutor>,
    name: String,
    pm_domain: String,
}

impl GrpcPMContextFactory {
    /// Create a new factory with gRPC client, command executor, and PM domain.
    pub fn new(
        client: Arc<Mutex<ProcessManagerClient<tonic::transport::Channel>>>,
        command_executor: Arc<dyn CommandExecutor>,
        name: String,
        pm_domain: String,
    ) -> Self {
        Self {
            client,
            command_executor,
            name,
            pm_domain,
        }
    }
}

impl PMContextFactory for GrpcPMContextFactory {
    fn create(&self) -> Box<dyn ProcessManagerContext> {
        Box::new(GrpcPMContext::new(
            self.client.clone(),
            self.command_executor.clone(),
            self.pm_domain.clone(),
        ))
    }

    fn pm_domain(&self) -> &str {
        &self.pm_domain
    }

    fn name(&self) -> &str {
        &self.name
    }
}
