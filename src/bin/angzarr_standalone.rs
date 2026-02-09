//! angzarr-standalone: All-in-one infrastructure host
//!
//! Runs all angzarr infrastructure in a single process while spawning
//! client logic processes as separate gRPC servers over UDS.
//!
//! ## Architecture
//! ```text
//! angzarr-standalone (single process, all infrastructure)
//!     │
//!     │  client logic processes (gRPC over UDS):
//!     ├── business-customer (Python)      → business-customer.sock
//!     ├── business-order (Python)         → business-order.sock
//!     ├── saga-fulfillment (Python)       → saga-fulfillment.sock
//!     ├── projector-web (Go)              → projector-web.sock
//!     │
//!     │  Infrastructure (all in-process, channels):
//!     ├── CommandRouter + ClientLogic ──gRPC/UDS──→ business-*.sock
//!     ├── ProjectorEventHandler       ──gRPC/UDS──→ projector-*.sock
//!     ├── SagaEventHandler            ──gRPC/UDS──→ saga-*.sock
//!     ├── EventBus (tokio broadcast channels)
//!     ├── Storage (direct access)
//!     └── Gateway (:50051 TCP for external clients)
//! ```
//!
//! ## Configuration
//! ```yaml
//! storage:
//!   type: sqlite
//!
//! transport:
//!   type: uds
//!   uds:
//!     base_path: /tmp/angzarr
//!
//! standalone:
//!   aggregates:
//!     - domain: customer
//!       command: uv run --directory customer python server.py
//!     - domain: order
//!       command: uv run --directory order python server.py
//!   sagas:
//!     - domain: fulfillment
//!       listen_domain: order
//!       command: uv run --directory saga-fulfillment python server.py
//!   process_managers:
//!     - domain: game-orchestrator
//!       listen_domain: game
//!       command: uv run --directory game-orchestrator python server.py
//!   projectors:
//!     - domain: web
//!       command: uv run --directory projector-web python server.py
//!   services:
//!     - name: api-server
//!       command: ./api-server
//!       args: ["--port", "8080"]
//!       health_check:
//!         type: http
//!         endpoint: http://localhost:8080/health
//!       health_timeout_secs: 30
//!   gateway:
//!     enabled: true
//!     port: 50051
//! ```

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use angzarr::bus::{ChannelConfig, ChannelEventBus, EventBus};
use angzarr::config::SagaCompensationConfig;
use angzarr::config::{
    Config, ExternalServiceConfig, HealthCheckConfig, CONFIG_ENV_PREFIX, CONFIG_ENV_VAR,
    TRANSPORT_TYPE_ENV_VAR,
};
use angzarr::discovery::k8s::K8sServiceDiscovery;
use angzarr::discovery::ServiceDiscovery;
use angzarr::handlers::core::{
    ProcessManagerEventHandler, ProjectorEventHandler, SagaEventHandler,
};
use angzarr::orchestration::aggregate::GrpcBusinessLogic;
use angzarr::orchestration::command::local::LocalCommandExecutor;
use angzarr::orchestration::command::CommandExecutor;
use angzarr::orchestration::destination::local::LocalDestinationFetcher;
use angzarr::orchestration::process_manager::grpc::GrpcPMContextFactory;
use angzarr::orchestration::saga::grpc::GrpcSagaContextFactory;
use angzarr::proto::aggregate_client::AggregateClient;
use angzarr::proto::command_gateway_server::CommandGatewayServer;
use angzarr::proto::event_query_server::EventQueryServer;
use angzarr::proto::process_manager_client::ProcessManagerClient;
use angzarr::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use angzarr::proto::saga_client::SagaClient;
use angzarr::standalone::{
    AggregateHandlerAdapter, CommandRouter, DomainStorage, GrpcProjectorHandler,
    MetaAggregateHandler, ServerInfo, StandaloneEventQueryBridge, StandaloneGatewayService,
    META_DOMAIN,
};
use angzarr::transport::{connect_to_address, grpc_trace_layer};

/// Managed child process with proper cleanup.
struct ManagedChild {
    child: Child,
    name: String,
}

impl ManagedChild {
    /// Spawn a client logic process from a command array.
    async fn spawn(
        name: &str,
        command: &[String],
        working_dir: Option<&str>,
        env: &HashMap<String, String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let executable = command
            .first()
            .ok_or_else(|| format!("Empty command for {}", name))?;
        let args = &command[1..];

        info!(name = %name, executable = %executable, "Spawning process");

        let mut cmd = Command::new(executable);
        cmd.args(args);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        #[cfg(unix)]
        cmd.process_group(0);

        let child = cmd.spawn().map_err(|e| {
            error!(name = %name, error = %e, "Failed to spawn process");
            e
        })?;

        info!(name = %name, pid = ?child.id(), "Process started");

        Ok(Self {
            child,
            name: name.to_string(),
        })
    }

    /// Kill the process and all its descendants.
    async fn kill(&mut self) {
        if let Some(pid) = self.child.id() {
            info!(name = %self.name, pid = pid, "Killing process");

            #[cfg(unix)]
            {
                use nix::sys::signal::{killpg, Signal};
                use nix::unistd::Pid;

                let pgid = Pid::from_raw(pid as i32);
                if let Err(e) = killpg(pgid, Signal::SIGTERM) {
                    warn!(name = %self.name, error = %e, "Failed to send SIGTERM to process group");
                }
            }

            let _ = self.child.start_kill();

            match tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await {
                Ok(Ok(status)) => {
                    info!(name = %self.name, status = ?status, "Process exited");
                }
                Ok(Err(e)) => {
                    warn!(name = %self.name, error = %e, "Error waiting for process");
                }
                Err(_) => {
                    warn!(name = %self.name, "Process didn't exit gracefully, sending SIGKILL");

                    #[cfg(unix)]
                    {
                        use nix::sys::signal::{killpg, Signal};
                        use nix::unistd::Pid;

                        let pgid = Pid::from_raw(pid as i32);
                        let _ = killpg(pgid, Signal::SIGKILL);
                    }

                    let _ = self.child.kill().await;
                }
            }
        }
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            if let Some(pid) = self.child.id() {
                #[cfg(unix)]
                {
                    use nix::sys::signal::{killpg, Signal};
                    use nix::unistd::Pid;

                    let pgid = Pid::from_raw(pid as i32);
                    let _ = killpg(pgid, Signal::SIGKILL);
                }
                let _ = self.child.start_kill();
            }
        }
    }
}

/// Resolve UDS socket path for a client logic process.
fn socket_path(base_path: &std::path::Path, prefix: &str, name: &str) -> String {
    format!("{}/{}-{}.sock", base_path.display(), prefix, name)
}

/// Wait for a UDS socket to become connectable.
async fn wait_for_socket(
    path: &str,
    name: &str,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();
    let interval = Duration::from_millis(100);

    info!(name = %name, path = %path, "Waiting for socket");

    loop {
        if start.elapsed() > timeout {
            return Err(format!(
                "Socket for '{}' not available after {:?}: {}",
                name, timeout, path
            )
            .into());
        }

        if tokio::net::UnixStream::connect(path).await.is_ok() {
            info!(
                name = %name,
                elapsed_ms = start.elapsed().as_millis(),
                "Socket ready"
            );
            return Ok(());
        }

        tokio::time::sleep(interval).await;
    }
}

/// Spawn a client logic process and wait for its UDS socket.
///
/// Returns immediately if command is empty (pre-existing process).
async fn spawn_client_process(
    name: &str,
    command: &[String],
    working_dir: Option<&str>,
    env: &HashMap<String, String>,
    socket_path: &str,
    children: &mut Vec<ManagedChild>,
) -> Result<(), Box<dyn std::error::Error>> {
    if command.is_empty() {
        return Ok(());
    }

    let mut process_env = env.clone();
    // Tell client logic servers to listen on UDS instead of TCP
    process_env.insert(TRANSPORT_TYPE_ENV_VAR.to_string(), "uds".to_string());
    process_env.insert(
        format!("{}__{}__{}", CONFIG_ENV_PREFIX, "TARGET", "ADDRESS"),
        socket_path.to_string(),
    );
    if let Some(config_path) = angzarr::utils::bootstrap::parse_config_path() {
        process_env.insert(CONFIG_ENV_VAR.to_string(), config_path);
    }

    let child = ManagedChild::spawn(name, command, working_dir, &process_env).await?;
    children.push(child);

    wait_for_socket(socket_path, name, Duration::from_secs(30)).await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    angzarr::utils::bootstrap::init_tracing();

    let config_path = angzarr::utils::bootstrap::parse_config_path();
    let config = Config::load(config_path.as_deref()).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-standalone");

    // Create UDS base directory and clean stale sockets
    let base_path = &config.transport.uds.base_path;
    tokio::fs::create_dir_all(base_path).await?;

    let mut entries = tokio::fs::read_dir(base_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "sock").unwrap_or(false) {
            tokio::fs::remove_file(&path).await.ok();
        }
    }
    info!(path = %base_path.display(), "UDS directory ready");

    let mut children: Vec<ManagedChild> = Vec::new();

    // Initialize per-domain storage
    let mut domain_stores: HashMap<String, DomainStorage> = HashMap::new();
    for svc in &config.standalone.aggregates {
        let storage_config = svc.storage.as_ref().unwrap_or(&config.storage);
        let (event_store, snapshot_store) = angzarr::storage::init_storage(storage_config).await?;

        info!(
            domain = %svc.domain,
            storage_type = ?storage_config.storage_type,
            "Initialized storage"
        );

        domain_stores.insert(
            svc.domain.clone(),
            DomainStorage {
                event_store,
                snapshot_store,
            },
        );
    }

    // Initialize storage for _angzarr meta domain
    {
        let (event_store, snapshot_store) = angzarr::storage::init_storage(&config.storage).await?;
        domain_stores.insert(
            META_DOMAIN.to_string(),
            DomainStorage {
                event_store,
                snapshot_store,
            },
        );
        info!(domain = %META_DOMAIN, "Initialized meta domain storage");
    }

    // Initialize PM storage early so it's available for LocalDestinationFetcher
    // PMs need their own storage since they emit their own events
    for svc in &config.standalone.process_managers {
        if !domain_stores.contains_key(&svc.domain) {
            let storage_config = svc.storage.as_ref().unwrap_or(&config.storage);
            let (event_store, snapshot_store) =
                angzarr::storage::init_storage(storage_config).await?;

            info!(
                domain = %svc.domain,
                storage_type = ?storage_config.storage_type,
                "Initialized PM storage"
            );

            domain_stores.insert(
                svc.domain.clone(),
                DomainStorage {
                    event_store,
                    snapshot_store,
                },
            );
        }
    }

    // Create channel event bus (in-process event distribution)
    let channel_bus = Arc::new(ChannelEventBus::new(ChannelConfig::publisher()));
    let event_bus: Arc<dyn EventBus> = channel_bus.clone();

    // Spawn and connect aggregate client logic processes
    let mut client_logic: HashMap<String, Arc<dyn angzarr::orchestration::aggregate::ClientLogic>> =
        HashMap::new();

    // Register built-in _angzarr meta aggregate (in-process, no external process)
    client_logic.insert(
        META_DOMAIN.to_string(),
        Arc::new(AggregateHandlerAdapter::new(Arc::new(
            MetaAggregateHandler::new(),
        ))),
    );

    for svc in &config.standalone.aggregates {
        let path = svc
            .address
            .clone()
            .unwrap_or_else(|| socket_path(base_path, "business", &svc.domain));

        spawn_client_process(
            &format!("business-{}", svc.domain),
            &svc.command,
            svc.working_dir.as_deref(),
            &svc.env,
            &path,
            &mut children,
        )
        .await?;

        let channel = connect_to_address(&path).await?;
        let client = AggregateClient::new(channel);
        client_logic.insert(svc.domain.clone(), Arc::new(GrpcBusinessLogic::new(client)));

        info!(domain = %svc.domain, "Connected to aggregate client logic");
    }

    // Service discovery (static — no K8s in standalone)
    let discovery: Arc<dyn ServiceDiscovery> = Arc::new(K8sServiceDiscovery::new_static());

    // Create command router (no in-process sync projectors in binary mode)
    let router = Arc::new(CommandRouter::new(
        client_logic,
        domain_stores.clone(),
        discovery,
        event_bus.clone(),
        vec![],
        None,
    ));

    // Create local command executor and destination fetcher
    let executor = Arc::new(LocalCommandExecutor::new(router.clone()));
    let fetcher = Arc::new(LocalDestinationFetcher::new(domain_stores.clone()));

    // Spawn and connect projector processes
    for svc in &config.standalone.projectors {
        let socket_name = match &svc.name {
            Some(name) => format!("{}-{}", name, svc.domain),
            None => svc.domain.clone(),
        };
        let path = svc
            .address
            .clone()
            .unwrap_or_else(|| socket_path(base_path, "projector", &socket_name));

        spawn_client_process(
            &format!("projector-{}", socket_name),
            &svc.command,
            svc.working_dir.as_deref(),
            &svc.env,
            &path,
            &mut children,
        )
        .await?;

        let channel = connect_to_address(&path).await?;
        let client = ProjectorCoordinatorClient::new(channel);
        let handler: Arc<dyn angzarr::standalone::ProjectorHandler> =
            Arc::new(GrpcProjectorHandler::new(client));

        let listen_domain = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
        let proj_handler = ProjectorEventHandler::with_config(
            handler,
            None,
            vec![listen_domain.clone()],
            false,
            socket_name.clone(),
        );

        let sub = channel_bus.with_config(ChannelConfig::subscriber_all());
        sub.subscribe(Box::new(proj_handler)).await?;
        sub.start_consuming().await?;

        info!(domain = %svc.domain, listen = %listen_domain, "Connected to projector");
    }

    // Spawn and connect saga processes
    let compensation_config: SagaCompensationConfig =
        config.saga_compensation.clone().unwrap_or_default();

    for svc in &config.standalone.sagas {
        let path = svc
            .address
            .clone()
            .unwrap_or_else(|| socket_path(base_path, "saga", &svc.domain));

        spawn_client_process(
            &format!("saga-{}", svc.domain),
            &svc.command,
            svc.working_dir.as_deref(),
            &svc.env,
            &path,
            &mut children,
        )
        .await?;

        let channel = connect_to_address(&path).await?;
        let saga_client = Arc::new(Mutex::new(SagaClient::new(channel)));
        let listen_domain = svc.listen_domain.as_ref().unwrap_or(&svc.domain);

        let factory = Arc::new(GrpcSagaContextFactory::new(
            saga_client,
            event_bus.clone(),
            compensation_config.clone(),
            None,
            svc.domain.clone(),
        ));

        let handler =
            SagaEventHandler::from_factory(factory, executor.clone(), Some(fetcher.clone()));

        let input_domain = listen_domain.clone();
        let sub = channel_bus.with_config(ChannelConfig::subscriber(input_domain));
        sub.subscribe(Box::new(handler)).await?;
        sub.start_consuming().await?;

        info!(domain = %svc.domain, listen = %listen_domain, "Connected to saga");
    }

    // Spawn and connect process manager processes
    // PM storage already initialized at startup (see earlier loop)
    for svc in &config.standalone.process_managers {
        let path = svc
            .address
            .clone()
            .unwrap_or_else(|| socket_path(base_path, "pm", &svc.domain));

        spawn_client_process(
            &format!("pm-{}", svc.domain),
            &svc.command,
            svc.working_dir.as_deref(),
            &svc.env,
            &path,
            &mut children,
        )
        .await?;

        let channel = connect_to_address(&path).await?;
        let mut pm_client = ProcessManagerClient::new(channel);

        // Get PM descriptor to determine which domains/event types it subscribes to
        let descriptor = pm_client
            .get_descriptor(angzarr::proto::GetDescriptorRequest {})
            .await?
            .into_inner();
        let subscriptions = descriptor.inputs.clone();

        info!(
            domain = %svc.domain,
            subscriptions = subscriptions.len(),
            "PM subscriptions from descriptor"
        );
        for sub in &subscriptions {
            info!(
                domain = %sub.domain,
                types = ?sub.types,
                "PM subscription target"
            );
        }

        let pm_client = Arc::new(Mutex::new(pm_client));

        // Get PM's event store for persisting PM state events
        let pm_storage = domain_stores
            .get(&svc.domain)
            .expect("PM storage must exist");

        let factory = Arc::new(GrpcPMContextFactory::new(
            pm_client,
            pm_storage.event_store.clone(),
            event_bus.clone(),
            svc.domain.clone(),
            svc.domain.clone(),
        ));

        // Use with_targets for handler-level filtering (PM receives all events, filters internally)
        let handler =
            ProcessManagerEventHandler::from_factory(factory, fetcher.clone(), executor.clone())
                .with_targets(subscriptions);

        // Subscribe to ALL events - PM does handler-level filtering via with_targets
        let sub = channel_bus.with_config(ChannelConfig::subscriber_all());
        sub.subscribe(Box::new(handler)).await?;
        sub.start_consuming().await?;

        info!(domain = %svc.domain, "Connected to process manager");
    }

    // Propagate config path to external services
    let config_path = angzarr::utils::bootstrap::parse_config_path();

    // Spawn external services (REST APIs, GraphQL servers, etc.)
    for svc in &config.standalone.services {
        let mut svc_with_env = svc.clone();
        if let Some(ref path) = config_path {
            svc_with_env
                .env
                .insert(CONFIG_ENV_VAR.to_string(), path.clone());
        }
        let child = spawn_external_service(&svc_with_env.name, &svc_with_env).await?;
        children.push(child);

        if !matches!(svc_with_env.health_check, HealthCheckConfig::None) {
            wait_for_service_health(
                &svc_with_env.name,
                &svc_with_env.health_check,
                svc_with_env.health_timeout_secs,
            )
            .await?;
        }
    }

    // Start gateway if enabled
    let mut servers: Vec<ServerInfo> = Vec::new();
    if config.standalone.gateway.enabled {
        let port = config.standalone.gateway.port.unwrap_or(50051);
        let addr: std::net::SocketAddr = format!("0.0.0.0:{port}").parse()?;

        let gateway = StandaloneGatewayService::new(router.clone());
        let event_query = StandaloneEventQueryBridge::new(domain_stores.clone());

        let grpc_router = tonic::transport::Server::builder()
            .layer(grpc_trace_layer())
            .add_service(CommandGatewayServer::new(gateway))
            .add_service(EventQueryServer::new(event_query));

        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        tokio::spawn(async move {
            let server = grpc_router.serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
            });
            if let Err(e) = server.await {
                error!(error = %e, "Gateway server error");
            }
        });

        servers.push(ServerInfo::from_parts(local_addr, shutdown_tx));
        info!(addr = %local_addr, "Gateway listening");
    }

    // Register components via _angzarr meta aggregate (command-based registration)
    let descriptors = build_descriptors(&config);
    let pod_id = angzarr::proto_ext::get_pod_id();
    let commands = angzarr::proto_ext::build_registration_commands(&descriptors, &pod_id);

    for cmd in &commands {
        match executor.execute(cmd.clone()).await {
            angzarr::orchestration::command::CommandOutcome::Success(_) => {}
            angzarr::orchestration::command::CommandOutcome::Retryable { reason, .. } => {
                warn!(reason = %reason, "Retryable error registering component");
            }
            angzarr::orchestration::command::CommandOutcome::Rejected(reason) => {
                warn!(reason = %reason, "Rejected while registering component");
            }
        }
    }
    info!(
        count = descriptors.len(),
        "Components registered via _angzarr"
    );

    // Spawn heartbeat for re-registration (handles topology startup race)
    let hb_executor = executor.clone();
    let strategy = config.standalone.registration.build_strategy();
    tokio::spawn(async move {
        let mut attempt = 0u32;
        loop {
            if let Some(delay) = strategy.next_delay(attempt) {
                tokio::time::sleep(delay).await;
                for cmd in &commands {
                    let _ = hb_executor.execute(cmd.clone()).await;
                }
                attempt = attempt.saturating_add(1);
            } else {
                info!("Registration heartbeat stopped after {} attempts", attempt);
                break;
            }
        }
    });

    info!(
        aggregates = config.standalone.aggregates.len(),
        sagas = config.standalone.sagas.len(),
        projectors = config.standalone.projectors.len(),
        process_managers = config.standalone.process_managers.len(),
        services = config.standalone.services.len(),
        gateway = config.standalone.gateway.enabled,
        "All components started"
    );

    info!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    info!("Shutting down...");

    // Shutdown gateway
    for server in servers {
        server.shutdown();
    }

    // Kill children in reverse order (LIFO: external services, then sagas, projectors, aggregates)
    for child in children.iter_mut().rev() {
        child.kill().await;
    }

    info!("Shutdown complete");
    Ok(())
}

/// Spawn an external service process.
async fn spawn_external_service(
    name: &str,
    svc: &ExternalServiceConfig,
) -> Result<ManagedChild, Box<dyn std::error::Error>> {
    if svc.command.is_empty() {
        return Err(format!("External service '{}' has empty command array", name).into());
    }

    let executable = &svc.command[0];
    let args = &svc.command[1..];

    info!(
        name = %name,
        executable = %executable,
        ?args,
        "Spawning external service"
    );

    let mut cmd = Command::new(executable);
    cmd.args(args);

    if let Some(ref dir) = svc.working_dir {
        cmd.current_dir(dir);
    }

    for (key, value) in &svc.env {
        cmd.env(key, value);
    }

    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    #[cfg(unix)]
    cmd.process_group(0);

    let child = cmd.spawn().map_err(|e| {
        error!(name = %name, executable = %executable, error = %e, "Failed to spawn external service");
        e
    })?;

    info!(name = %name, pid = ?child.id(), "External service started");

    Ok(ManagedChild {
        child,
        name: name.to_string(),
    })
}

/// Wait for an external service to become healthy.
async fn wait_for_service_health(
    name: &str,
    health_check: &HealthCheckConfig,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let timeout = Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(500);
    let start = std::time::Instant::now();

    info!(
        name = %name,
        timeout_secs = timeout_secs,
        "Waiting for external service health check"
    );

    loop {
        if start.elapsed() > timeout {
            return Err(format!(
                "External service '{}' health check timed out after {:?}",
                name, timeout
            )
            .into());
        }

        let healthy = match health_check {
            HealthCheckConfig::None => true,
            HealthCheckConfig::Http { endpoint } => check_http_health(endpoint).await,
            HealthCheckConfig::Tcp { address } => check_tcp_health(address).await,
            HealthCheckConfig::Grpc { address } => check_grpc_health(address).await,
        };

        if healthy {
            info!(
                name = %name,
                elapsed_ms = start.elapsed().as_millis(),
                "External service health check passed"
            );
            return Ok(());
        }

        tokio::time::sleep(interval).await;
    }
}

/// Check HTTP health endpoint.
async fn check_http_health(url: &str) -> bool {
    use std::io::{Read, Write};

    let url = match url.strip_prefix("http://") {
        Some(rest) => rest,
        None => match url.strip_prefix("https://") {
            Some(_) => {
                warn!("HTTPS health checks not supported, use TCP or HTTP");
                return false;
            }
            None => url,
        },
    };

    let (host_port, path) = match url.find('/') {
        Some(idx) => (&url[..idx], &url[idx..]),
        None => (url, "/"),
    };

    let stream = match std::net::TcpStream::connect_timeout(
        &host_port
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
        Duration::from_secs(5),
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let mut stream = stream;

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host_port
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = [0u8; 1024];
    match stream.read(&mut response) {
        Ok(n) if n > 0 => {
            let response_str = String::from_utf8_lossy(&response[..n]);
            response_str.contains("200 ") || response_str.contains("204 ")
        }
        _ => false,
    }
}

/// Check TCP connection health.
async fn check_tcp_health(addr: &str) -> bool {
    tokio::net::TcpStream::connect(addr).await.is_ok()
}

/// Check gRPC health (TCP connectivity).
async fn check_grpc_health(addr: &str) -> bool {
    check_tcp_health(addr).await
}

/// Build component descriptors from standalone config for topology discovery.
fn build_descriptors(config: &angzarr::config::Config) -> Vec<angzarr::proto::ComponentDescriptor> {
    let mut descriptors = Vec::new();

    for svc in &config.standalone.aggregates {
        descriptors.push(angzarr::proto::ComponentDescriptor {
            name: svc.domain.clone(),
            component_type: "aggregate".to_string(),
            inputs: vec![],
        });
    }

    for svc in &config.standalone.sagas {
        let listen = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
        descriptors.push(angzarr::proto::ComponentDescriptor {
            name: svc.domain.clone(),
            component_type: "saga".to_string(),
            inputs: vec![angzarr::proto::Target {
                domain: listen.clone(),
                types: vec![],
            }],
        });
    }

    for svc in &config.standalone.projectors {
        let listen = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
        descriptors.push(angzarr::proto::ComponentDescriptor {
            name: svc.name.clone().unwrap_or_else(|| svc.domain.clone()),
            component_type: "projector".to_string(),
            inputs: vec![angzarr::proto::Target {
                domain: listen.clone(),
                types: vec![],
            }],
        });
    }

    for svc in &config.standalone.process_managers {
        let listen = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
        descriptors.push(angzarr::proto::ComponentDescriptor {
            name: svc.domain.clone(),
            component_type: "process_manager".to_string(),
            inputs: vec![angzarr::proto::Target {
                domain: listen.clone(),
                types: vec![],
            }],
        });
    }

    descriptors
}
