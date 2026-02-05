//! angzarr-process-manager: Process Manager sidecar
//!
//! Kubernetes sidecar for process manager services. Subscribes to events from
//! multiple domains via the message bus, coordinates long-running workflows
//! with event-sourced state.
//!
//! ## Two-Phase Protocol
//! 1. **Prepare**: PM declares additional destinations needed (beyond trigger)
//! 2. **Fetch**: Sidecar fetches destination EventBooks via EventQuery
//! 3. **Handle**: PM receives trigger + PM state + destinations, produces commands + PM events
//!
//! ## Differences from Saga
//! - PM subscribes to MULTIPLE domains (saga recommends single domain)
//! - PM maintains event-sourced state in its own domain
//! - PM calls GetSubscriptions at startup to configure routing
//!
//! ## State Persistence
//! PM state events are persisted directly to the event store and published
//! to the event bus, bypassing the command pipeline. This avoids needing
//! an aggregate sidecar for the PM's own domain.
//!
//! ## Configuration
//! - TARGET_ADDRESS: ProcessManager gRPC address (e.g., "localhost:50060")
//! - TARGET_DOMAIN: Process manager domain name (used for PM state storage)
//! - TARGET_COMMAND: Optional command to spawn PM (embedded mode)
//! - ANGZARR_STATIC_ENDPOINTS: Static endpoints for multi-domain routing
//! - MESSAGING_TYPE: amqp, kafka, or ipc

use std::sync::Arc;
use std::time::Duration;

use backon::Retryable;
use tracing::{info, warn};

use angzarr::bus::{AmqpConfig, AmqpEventBus, EventBus, IpcConfig, IpcEventBus, MessagingType, MockEventBus};
use angzarr::config::STATIC_ENDPOINTS_ENV_VAR;
use angzarr::handlers::core::ProcessManagerEventHandler;
use angzarr::orchestration::destination::hybrid::HybridDestinationFetcher;
use angzarr::proto::process_manager_client::ProcessManagerClient;
use angzarr::proto::GetDescriptorRequest;
use angzarr::storage::init_storage;
use angzarr::transport::connect_to_address;
use angzarr::utils::retry::connection_backoff;
use angzarr::utils::sidecar::{bootstrap_sidecar, connect_endpoints, run_subscriber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = bootstrap_sidecar("process-manager").await?;

    let messaging = bootstrap
        .config
        .messaging
        .as_ref()
        .ok_or("Process manager sidecar requires 'messaging' configuration")?;

    info!(messaging_type = ?messaging.messaging_type, "Using messaging backend");

    // Initialize storage for direct PM state persistence
    let (event_store, _snapshot_store) = init_storage(&bootstrap.config.storage).await?;
    info!("PM storage initialized for direct state persistence");

    // Initialize event bus (publisher) for PM state events
    let event_bus: Arc<dyn EventBus> = match messaging.messaging_type {
        MessagingType::Amqp => {
            let amqp_config = AmqpConfig::publisher(&messaging.amqp.url);
            Arc::new(AmqpEventBus::new(amqp_config).await?)
        }
        MessagingType::Ipc => {
            let ipc_config = IpcConfig::publisher(&messaging.ipc.base_path);
            Arc::new(IpcEventBus::new(ipc_config))
        }
        _ => {
            warn!("No messaging configured for PM event publishing, using mock");
            Arc::new(MockEventBus::new())
        }
    };

    // Connect to process manager service
    let pm_addr = bootstrap.address.clone();
    let mut pm_client = (|| {
        let addr = pm_addr.clone();
        async move {
            let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(ProcessManagerClient::new(channel))
        }
    })
    .retry(connection_backoff())
    .notify(|err: &String, dur: Duration| {
        warn!(service = "process-manager", error = %err, delay = ?dur, "Connection failed, retrying");
    })
    .await?;

    // Get component descriptor from process manager
    let descriptor = pm_client
        .get_descriptor(GetDescriptorRequest {})
        .await?
        .into_inner();

    let subscriptions = descriptor.inputs.clone();
    info!(
        name = %descriptor.name,
        component_type = %descriptor.component_type,
        subscriptions = subscriptions.len(),
        outputs = descriptor.outputs.len(),
        "Process manager descriptor"
    );

    for sub in &subscriptions {
        info!(
            domain = %sub.domain,
            types = ?sub.types,
            "Input target"
        );
    }

    // Write descriptor to pod annotation for K8s-native topology discovery
    if let Err(e) = angzarr::discovery::k8s::write_descriptor_if_k8s(&descriptor).await {
        warn!(error = %e, "Failed to write descriptor annotation");
    }

    // Connect to all aggregate endpoints (business domains only)
    let endpoints_str = std::env::var(STATIC_ENDPOINTS_ENV_VAR).map_err(|_| {
        format!("Process manager requires {} for multi-domain routing", STATIC_ENDPOINTS_ENV_VAR)
    })?;

    let (command_executor, remote_fetcher) = connect_endpoints(&endpoints_str).await?;

    // Wrap the remote fetcher with hybrid that handles PM domain locally
    let hybrid_fetcher = Arc::new(HybridDestinationFetcher::new(
        bootstrap.domain.clone(),
        event_store.clone(),
        remote_fetcher,
    ));

    // Create handler with direct storage for PM state persistence
    let handler = ProcessManagerEventHandler::new(
        pm_client,
        bootstrap.domain.clone(),
        hybrid_fetcher,
        command_executor,
        event_store,
        event_bus,
    );

    let queue_name = format!("process-manager-{}", bootstrap.domain);
    run_subscriber(messaging, queue_name, Box::new(handler)).await
}
