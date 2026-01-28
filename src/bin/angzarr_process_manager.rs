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
//! ## Configuration
//! - TARGET_ADDRESS: ProcessManager gRPC address (e.g., "localhost:50060")
//! - TARGET_DOMAIN: Process manager domain name (used for PM state storage)
//! - TARGET_COMMAND: Optional command to spawn PM (embedded mode)
//! - ANGZARR_STATIC_ENDPOINTS: Static endpoints for multi-domain routing
//! - MESSAGING_TYPE: amqp, kafka, or ipc

use tracing::info;

use angzarr::handlers::core::ProcessManagerEventHandler;
use angzarr::proto::process_manager_client::ProcessManagerClient;
use angzarr::proto::GetSubscriptionsRequest;
use angzarr::transport::connect_to_address;
use angzarr::utils::bootstrap::connect_with_retry;
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

    // Connect to process manager service
    let pm_addr = bootstrap.address.clone();
    let mut pm_client = connect_with_retry("process-manager", &bootstrap.address, || {
        let addr = pm_addr.clone();
        async move {
            let channel = connect_to_address(&addr).await.map_err(|e| e.to_string())?;
            Ok::<_, String>(ProcessManagerClient::new(channel))
        }
    })
    .await?;

    // Get subscriptions from process manager
    let subscriptions_response = pm_client
        .get_subscriptions(GetSubscriptionsRequest {})
        .await?
        .into_inner();

    let subscriptions = subscriptions_response.subscriptions;
    info!(
        subscriptions = subscriptions.len(),
        "Process manager declared subscriptions"
    );

    for sub in &subscriptions {
        info!(
            domain = %sub.domain,
            event_types = ?sub.event_types,
            "Subscription"
        );
    }

    // Connect to all aggregate endpoints
    let endpoints_str = std::env::var("ANGZARR_STATIC_ENDPOINTS").map_err(|_| {
        "Process manager requires ANGZARR_STATIC_ENDPOINTS for multi-domain routing"
    })?;

    let (command_executor, destination_fetcher) = connect_endpoints(&endpoints_str).await?;

    // Create handler
    let handler = ProcessManagerEventHandler::new(
        pm_client,
        bootstrap.domain.clone(),
        destination_fetcher,
        command_executor,
    );

    let queue_name = format!("process-manager-{}", bootstrap.domain);
    run_subscriber(messaging, queue_name, Box::new(handler)).await
}
