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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info, warn};

use angzarr::bus::{init_event_bus, EventBusMode};
use angzarr::config::Config;
use angzarr::handlers::core::ProcessManagerEventHandler;
use angzarr::orchestration::command::grpc::GrpcCommandExecutor;
use angzarr::orchestration::command::CommandExecutor;
use angzarr::orchestration::destination::grpc::GrpcDestinationFetcher;
use angzarr::orchestration::destination::DestinationFetcher;
use angzarr::process::{wait_for_ready, ManagedProcess, ProcessEnv};
use angzarr::proto::aggregate_coordinator_client::AggregateCoordinatorClient;
use angzarr::proto::event_query_client::EventQueryClient;
use angzarr::proto::process_manager_client::ProcessManagerClient;
use angzarr::proto::GetSubscriptionsRequest;
use angzarr::transport::connect_to_address;
use angzarr::utils::bootstrap::{connect_with_retry, init_tracing, parse_static_endpoints};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-process-manager sidecar");

    let target = config
        .target
        .as_ref()
        .ok_or("Process manager sidecar requires 'target' configuration")?;

    // Process manager domain name (for state storage)
    let pm_domain = &target.domain;

    // Resolve address
    let address = target.resolve_address(&config.transport, "process-manager");

    info!(
        "Target process manager: {} (domain: {})",
        address, pm_domain
    );

    // Get command: prefer env var, fall back to config
    let command = match std::env::var("ANGZARR__TARGET__COMMAND_JSON") {
        Ok(json) => serde_json::from_str::<Vec<String>>(&json).unwrap_or_else(|_| {
            warn!("Failed to parse ANGZARR__TARGET__COMMAND_JSON, falling back to config");
            target.command.clone()
        }),
        Err(_) => target.command.clone(),
    };

    // Spawn PM process if command is configured (embedded mode)
    let _managed_process = if !command.is_empty() {
        let env = ProcessEnv::from_transport(&config.transport, "process-manager", Some(pm_domain));
        let process =
            ManagedProcess::spawn(&command, target.working_dir.as_deref(), &env, None).await?;

        info!("Waiting for process manager to be ready...");
        wait_for_ready(
            &address,
            Duration::from_secs(30),
            Duration::from_millis(500),
        )
        .await?;

        Some(process)
    } else {
        None
    };

    let messaging = config
        .messaging
        .as_ref()
        .ok_or("Process manager sidecar requires 'messaging' configuration")?;

    info!(messaging_type = ?messaging.messaging_type, "Using messaging backend");

    // Connect to process manager service
    let pm_addr = address.clone();
    let mut pm_client = connect_with_retry("process-manager", &address, || {
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

    // Get static endpoints for multi-domain routing
    let endpoints_str = std::env::var("ANGZARR_STATIC_ENDPOINTS").map_err(|_| {
        "Process manager requires ANGZARR_STATIC_ENDPOINTS for multi-domain routing"
    })?;

    let endpoints = parse_static_endpoints(&endpoints_str);

    // Connect to all aggregates
    let mut command_clients = HashMap::new();
    let mut query_clients = HashMap::new();

    for (domain, endpoint_address) in endpoints {
        // Connect AggregateCoordinator client
        let addr = endpoint_address.clone();
        let cmd_client =
            connect_with_retry(&format!("aggregate-{}", domain), &endpoint_address, || {
                let a = addr.clone();
                async move {
                    let channel = connect_to_address(&a).await.map_err(|e| e.to_string())?;
                    Ok::<_, String>(AggregateCoordinatorClient::new(channel))
                }
            })
            .await?;
        command_clients.insert(domain.clone(), cmd_client);

        // Connect EventQuery client
        let addr = endpoint_address.clone();
        let query_client = connect_with_retry(
            &format!("event-query-{}", domain),
            &endpoint_address,
            || {
                let a = addr.clone();
                async move {
                    let channel = connect_to_address(&a).await.map_err(|e| e.to_string())?;
                    Ok::<_, String>(EventQueryClient::new(channel))
                }
            },
        )
        .await?;
        query_clients.insert(domain.clone(), query_client);

        info!(domain = %domain, address = %endpoint_address, "Connected to aggregate");
    }

    let command_executor: Arc<dyn CommandExecutor> =
        Arc::new(GrpcCommandExecutor::new(command_clients));
    let destination_fetcher: Arc<dyn DestinationFetcher> =
        Arc::new(GrpcDestinationFetcher::new(query_clients));

    // Create handler
    let handler = ProcessManagerEventHandler::new(
        pm_client,
        pm_domain.clone(),
        destination_fetcher,
        command_executor,
    );

    // Subscribe to events from declared domains
    // Using SubscriberAll for now - PM handler filters by correlation_id
    // A future optimization could add SubscriberDomains mode to the bus
    let subscribed_domains: Vec<String> = subscriptions.iter().map(|s| s.domain.clone()).collect();

    let queue_name = format!("process-manager-{}", pm_domain);
    let subscriber_mode = EventBusMode::SubscriberAll { queue: queue_name };

    let subscriber = init_event_bus(messaging, subscriber_mode)
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    subscriber
        .subscribe(Box::new(handler))
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    subscriber
        .start_consuming()
        .await
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    info!(
        domains = ?subscribed_domains,
        "Process manager sidecar running, press Ctrl+C to exit"
    );

    tokio::signal::ctrl_c().await?;

    Ok(())
}
