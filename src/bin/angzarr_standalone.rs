//! angzarr-standalone: Standalone mode orchestrator
//!
//! Spawns and manages all sidecar processes for local development.
//! Replaces K8s orchestration with a single binary.
//!
//! ## Architecture
//! ```text
//! angzarr-standalone (orchestrator)
//!     │
//!     ├── angzarr-aggregate (customer) ──► business-customer.sock
//!     ├── angzarr-aggregate (order)    ──► business-order.sock
//!     ├── angzarr-saga (fulfillment)   ──► saga-fulfillment.sock
//!     ├── angzarr-projector (web)      ──► projector-web.sock
//!     ├── angzarr-stream               ──► stream.sock
//!     ├── angzarr-projector (stream)   ──► (feeds events to stream)
//!     └── angzarr-gateway              ──► :50051 (streams via stream.sock)
//! ```
//!
//! Each sidecar spawns its own business logic process via target.command.
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
//! messaging:
//!   type: amqp
//!   amqp:
//!     url: amqp://guest:guest@localhost:5672
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

use tokio::process::{Child, Command};
use tracing::{error, info, warn};

use angzarr::bus::{IpcBroker, IpcBrokerConfig, MessagingType, SUBSCRIBERS_ENV_VAR};
use angzarr::config::{Config, ExternalServiceConfig, HealthCheckConfig, ServiceConfig};

/// Managed child process with proper cleanup.
struct ManagedChild {
    child: Child,
    name: String,
}

impl ManagedChild {
    async fn spawn(
        name: &str,
        binary: &str,
        env: HashMap<String, String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!(name = %name, binary = %binary, "Spawning sidecar");

        // Log EVENT_QUERY_ADDRESS if present for debugging
        if let Some(addr) = env.get("EVENT_QUERY_ADDRESS") {
            info!(
                name = %name,
                event_query_address = %addr,
                "Setting EVENT_QUERY_ADDRESS for sidecar"
            );
        }

        // Log total env var count for debugging
        tracing::debug!(name = %name, env_count = env.len(), "Environment variables for sidecar");

        let mut cmd = Command::new(binary);
        // Clear ANGZARR_CONFIG so sidecars don't load the standalone config file
        cmd.env_remove("ANGZARR_CONFIG");
        for (key, value) in &env {
            cmd.env(key, value);
        }

        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        // Create new process group so we can kill all descendants
        #[cfg(unix)]
        cmd.process_group(0);

        let child = cmd.spawn().map_err(|e| {
            error!(name = %name, binary = %binary, error = %e, "Failed to spawn sidecar");
            e
        })?;

        info!(name = %name, pid = ?child.id(), "Sidecar started");

        Ok(Self {
            child,
            name: name.to_string(),
        })
    }

    /// Kill the process and all its descendants.
    async fn kill(&mut self) {
        if let Some(pid) = self.child.id() {
            info!(name = %self.name, pid = pid, "Killing sidecar");

            // Kill the process group (negative PID kills all processes in group)
            #[cfg(unix)]
            {
                use nix::sys::signal::{killpg, Signal};
                use nix::unistd::Pid;

                // Try SIGTERM first for graceful shutdown
                let pgid = Pid::from_raw(pid as i32);
                if let Err(e) = killpg(pgid, Signal::SIGTERM) {
                    warn!(name = %self.name, error = %e, "Failed to send SIGTERM to process group");
                }
            }

            // Also start tokio's kill
            let _ = self.child.start_kill();

            // Wait briefly for graceful shutdown
            match tokio::time::timeout(std::time::Duration::from_secs(2), self.child.wait()).await {
                Ok(Ok(status)) => {
                    info!(name = %self.name, status = ?status, "Sidecar exited");
                }
                Ok(Err(e)) => {
                    warn!(name = %self.name, error = %e, "Error waiting for sidecar");
                }
                Err(_) => {
                    // Timeout - force kill
                    warn!(name = %self.name, "Sidecar didn't exit gracefully, sending SIGKILL");

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
        // Synchronous fallback - try to start the kill process
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

/// Strip AMQP routing wildcards from a domain pattern.
///
/// Routing patterns like `game.#` or `game.*` should not be used
/// directly in socket paths. This extracts the base domain.
///
/// Examples:
/// - `game.#` -> `game`
/// - `game.*` -> `game`
/// - `game.player.*` -> `game.player`
/// - `game` -> `game` (unchanged)
fn strip_routing_wildcards(domain: &str) -> &str {
    domain
        .strip_suffix(".#")
        .or_else(|| domain.strip_suffix(".*"))
        .unwrap_or(domain)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    angzarr::utils::bootstrap::init_tracing();

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-standalone orchestrator");

    // Create UDS base directory
    let base_path = &config.transport.uds.base_path;
    tokio::fs::create_dir_all(base_path).await?;

    // Clean up stale sockets
    let mut entries = tokio::fs::read_dir(base_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "sock").unwrap_or(false) {
            tokio::fs::remove_file(&path).await.ok();
        }
    }
    info!(path = %base_path.display(), "UDS directory ready");

    // Find sidecar binaries (same directory as this binary, or in PATH)
    let self_path = std::env::current_exe()?;
    let bin_dir = self_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    let aggregate_bin = find_binary(bin_dir, "angzarr-aggregate")?;
    let saga_bin = find_binary(bin_dir, "angzarr-saga")?;
    let projector_bin = find_binary(bin_dir, "angzarr-projector")?;
    let gateway_bin = find_binary(bin_dir, "angzarr-gateway")?;
    let stream_bin = find_binary(bin_dir, "angzarr-stream")?;

    // Track all managed processes
    let mut children: Vec<ManagedChild> = Vec::new();

    // Base environment for all sidecars
    let base_env = build_base_env(&config);

    // Set up IPC broker if using IPC messaging
    let ipc_broker = setup_ipc_broker(&config).await?;
    let subscribers_json = ipc_broker
        .as_ref()
        .map(|b| b.subscribers_to_json())
        .unwrap_or_else(|| "[]".to_string());

    // Spawn aggregate sidecars (with subscriber list for IPC)
    for svc in &config.standalone.aggregates {
        let mut env = build_aggregate_env(&base_env, &config, svc)?;
        // Pass subscriber list for IPC fanout
        if ipc_broker.is_some() {
            env.insert(SUBSCRIBERS_ENV_VAR.to_string(), subscribers_json.clone());
        }
        let name = format!("aggregate-{}", svc.domain);
        let child = ManagedChild::spawn(&name, &aggregate_bin, env).await?;
        children.push(child);
    }

    // Spawn saga sidecars
    for svc in &config.standalone.sagas {
        let env = build_saga_env(&base_env, &config, svc, &ipc_broker)?;
        let name = format!("saga-{}", svc.domain);
        let child = ManagedChild::spawn(&name, &saga_bin, env).await?;
        children.push(child);
    }

    // Spawn projector sidecars
    for svc in &config.standalone.projectors {
        let env = build_projector_env(&base_env, &config, svc, &ipc_broker)?;
        let name = match &svc.name {
            Some(proj_name) => format!("projector-{}-{}", proj_name, svc.domain),
            None => format!("projector-{}", svc.domain),
        };
        let child = ManagedChild::spawn(&name, &projector_bin, env).await?;
        children.push(child);
    }

    // Spawn external services (REST APIs, GraphQL servers, etc.)
    // These services read from projection databases and serve data to clients
    for svc in &config.standalone.services {
        let child = spawn_external_service(&svc.name, svc).await?;
        children.push(child);

        // Wait for health check if configured
        if !matches!(svc.health_check, HealthCheckConfig::None) {
            wait_for_service_health(&svc.name, &svc.health_check, svc.health_timeout_secs).await?;
        }
    }

    // Spawn stream service if gateway is enabled
    // Stream enables execute_stream() for real-time event streaming to clients
    // Note: The stream projector must be configured explicitly in standalone.projectors
    if config.standalone.gateway.enabled {
        let stream_env = build_stream_env(&base_env, &config);
        let child = ManagedChild::spawn("stream", &stream_bin, stream_env).await?;
        children.push(child);
    }

    // Spawn gateway if enabled
    if config.standalone.gateway.enabled {
        let env = build_gateway_env(&base_env, &config);
        let child = ManagedChild::spawn("gateway", &gateway_bin, env).await?;
        children.push(child);
    }

    info!(
        aggregates = config.standalone.aggregates.len(),
        sagas = config.standalone.sagas.len(),
        projectors = config.standalone.projectors.len(),
        services = config.standalone.services.len(),
        gateway = config.standalone.gateway.enabled,
        streaming = config.standalone.gateway.enabled,
        "All sidecars started"
    );

    info!("Press Ctrl+C to exit");
    tokio::signal::ctrl_c().await?;

    info!("Shutting down {} children...", children.len());

    // Kill in reverse order (LIFO): gateway first, then projectors, sagas, aggregates
    for child in children.iter_mut().rev() {
        child.kill().await;
    }

    info!("All children terminated");
    Ok(())
}

/// Find a binary in the given directory or PATH.
fn find_binary(
    bin_dir: &std::path::Path,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Try same directory first
    let local_path = bin_dir.join(name);
    if local_path.exists() {
        return Ok(local_path.to_string_lossy().to_string());
    }

    // Try with .exe on Windows
    #[cfg(windows)]
    {
        let exe_path = bin_dir.join(format!("{}.exe", name));
        if exe_path.exists() {
            return Ok(exe_path.to_string_lossy().to_string());
        }
    }

    // Fall back to PATH
    Ok(name.to_string())
}

/// Build base environment variables from config (transport and messaging only).
fn build_base_env(config: &Config) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Transport
    env.insert("ANGZARR__TRANSPORT__TYPE".to_string(), "uds".to_string());
    env.insert(
        "ANGZARR__TRANSPORT__UDS__BASE_PATH".to_string(),
        config.transport.uds.base_path.to_string_lossy().to_string(),
    );

    // Messaging
    if let Some(ref messaging) = config.messaging {
        env.insert(
            "ANGZARR__MESSAGING__TYPE".to_string(),
            format!("{:?}", messaging.messaging_type).to_lowercase(),
        );

        match messaging.messaging_type {
            MessagingType::Ipc => {
                env.insert(
                    "ANGZARR__MESSAGING__IPC__BASE_PATH".to_string(),
                    messaging.ipc.base_path.clone(),
                );
            }
            MessagingType::Amqp => {
                env.insert(
                    "ANGZARR__MESSAGING__AMQP__URL".to_string(),
                    messaging.amqp.url.clone(),
                );
            }
            _ => {}
        }
    }

    env
}

/// Build storage environment variables from StorageConfig.
fn build_storage_env(env: &mut HashMap<String, String>, storage: &angzarr::storage::StorageConfig) {
    use angzarr::storage::StorageType;

    env.insert(
        "ANGZARR__STORAGE__TYPE".to_string(),
        format!("{:?}", storage.storage_type).to_lowercase(),
    );

    match storage.storage_type {
        StorageType::Sqlite => {
            if let Some(ref path) = storage.sqlite.path {
                env.insert("ANGZARR__STORAGE__SQLITE__PATH".to_string(), path.clone());
            }
        }
        StorageType::Postgres => {
            env.insert(
                "ANGZARR__STORAGE__POSTGRES__URI".to_string(),
                storage.postgres.uri.clone(),
            );
        }
        StorageType::Mongodb => {
            env.insert(
                "ANGZARR__STORAGE__MONGODB__URI".to_string(),
                storage.mongodb.uri.clone(),
            );
            env.insert(
                "ANGZARR__STORAGE__MONGODB__DATABASE".to_string(),
                storage.mongodb.database.clone(),
            );
        }
        StorageType::Redis => {
            env.insert(
                "ANGZARR__STORAGE__REDIS__URI".to_string(),
                storage.redis.uri.clone(),
            );
        }
    }
}

/// Build environment for aggregate sidecar.
fn build_aggregate_env(
    base: &HashMap<String, String>,
    config: &Config,
    svc: &ServiceConfig,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

    // Storage is required for each aggregate
    let storage = svc.storage.as_ref().ok_or_else(|| {
        format!(
            "Aggregate '{}' missing required storage configuration",
            svc.domain
        )
    })?;
    build_storage_env(&mut env, storage);

    // Target configuration
    env.insert(
        "ANGZARR__TARGET__ADDRESS".to_string(),
        format!("{}/business-{}.sock", base_path.display(), svc.domain),
    );
    env.insert("ANGZARR__TARGET__DOMAIN".to_string(), svc.domain.clone());
    // Pass command as JSON array for sidecar's ManagedProcess to parse
    if !svc.command.is_empty() {
        env.insert(
            "ANGZARR__TARGET__COMMAND_JSON".to_string(),
            serde_json::to_string(&svc.command).unwrap_or_default(),
        );
    }
    if let Some(ref working_dir) = svc.working_dir {
        env.insert(
            "ANGZARR__TARGET__WORKING_DIR".to_string(),
            working_dir.clone(),
        );
    }

    // Disable K8s service discovery in standalone mode
    env.insert("ANGZARR_DISCOVERY".to_string(), "static".to_string());

    Ok(env)
}

/// Set up IPC broker if using IPC messaging.
async fn setup_ipc_broker(
    config: &Config,
) -> Result<Option<IpcBroker>, Box<dyn std::error::Error>> {
    let messaging_type = config
        .messaging
        .as_ref()
        .map(|m| &m.messaging_type)
        .unwrap_or(&MessagingType::Amqp);

    if *messaging_type != MessagingType::Ipc {
        return Ok(None);
    }

    let base_path = config
        .messaging
        .as_ref()
        .map(|m| m.ipc.base_path.clone())
        .unwrap_or_else(|| "/tmp/angzarr".to_string());

    let broker_config = IpcBrokerConfig::with_base_path(&base_path);
    let mut broker = IpcBroker::new(broker_config);

    info!(base_path = %base_path, "Setting up IPC broker");

    // Register all projectors as subscribers
    for svc in &config.standalone.projectors {
        let name = match &svc.name {
            Some(proj_name) => format!("projector-{}-{}", proj_name, svc.domain),
            None => format!("projector-{}", svc.domain),
        };
        let domain = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
        broker.register_subscriber(&name, vec![domain.clone()])?;
    }

    // Register all sagas as subscribers
    for svc in &config.standalone.sagas {
        let name = format!("saga-{}", svc.domain);
        let domain = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
        broker.register_subscriber(&name, vec![domain.clone()])?;
    }

    info!(
        subscribers = broker.get_subscribers().len(),
        "IPC broker ready"
    );

    Ok(Some(broker))
}

/// Build environment for saga sidecar.
fn build_saga_env(
    base: &HashMap<String, String>,
    config: &Config,
    svc: &ServiceConfig,
    ipc_broker: &Option<IpcBroker>,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;
    let subscriber_name = format!("saga-{}", svc.domain);

    // Storage is required for each saga
    let storage = svc.storage.as_ref().ok_or_else(|| {
        format!(
            "Saga '{}' missing required storage configuration",
            svc.domain
        )
    })?;
    build_storage_env(&mut env, storage);

    // Target configuration
    env.insert(
        "ANGZARR__TARGET__ADDRESS".to_string(),
        format!("{}/saga-{}.sock", base_path.display(), svc.domain),
    );
    env.insert("ANGZARR__TARGET__DOMAIN".to_string(), svc.domain.clone());
    // Pass command as JSON array for sidecar's ManagedProcess to parse
    if !svc.command.is_empty() {
        env.insert(
            "ANGZARR__TARGET__COMMAND_JSON".to_string(),
            serde_json::to_string(&svc.command).unwrap_or_default(),
        );
    }
    if let Some(ref working_dir) = svc.working_dir {
        env.insert(
            "ANGZARR__TARGET__WORKING_DIR".to_string(),
            working_dir.clone(),
        );
    }

    // Messaging configuration - IPC or AMQP
    let listen_domain = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
    if ipc_broker.is_some() {
        // IPC mode: set subscriber name and domain
        env.insert(
            "ANGZARR__MESSAGING__IPC__SUBSCRIBER_NAME".to_string(),
            subscriber_name,
        );
        env.insert(
            "ANGZARR__MESSAGING__IPC__DOMAIN".to_string(),
            listen_domain.clone(),
        );
    } else {
        // AMQP mode: set domain
        env.insert(
            "ANGZARR__MESSAGING__AMQP__DOMAIN".to_string(),
            listen_domain.clone(),
        );
    }

    // Static endpoints for command routing (same format as gateway)
    let endpoints: Vec<String> = config
        .standalone
        .aggregates
        .iter()
        .map(|agg| {
            format!(
                "{}={}/aggregate-{}.sock",
                agg.domain,
                base_path.display(),
                agg.domain
            )
        })
        .collect();

    env.insert("ANGZARR_STATIC_ENDPOINTS".to_string(), endpoints.join(","));

    // EventQuery address for repair (use listen domain's aggregate)
    // Strip routing wildcards (e.g., "game.#" -> "game") for socket paths
    let repair_domain = strip_routing_wildcards(listen_domain);
    env.insert(
        "EVENT_QUERY_ADDRESS".to_string(),
        format!("{}/aggregate-{}.sock", base_path.display(), repair_domain),
    );

    // Merge in service-specific environment variables from config
    // These override any previously set values
    for (key, value) in &svc.env {
        env.insert(key.clone(), value.clone());
    }

    Ok(env)
}

/// Build environment for projector sidecar.
fn build_projector_env(
    base: &HashMap<String, String>,
    config: &Config,
    svc: &ServiceConfig,
    ipc_broker: &Option<IpcBroker>,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

    // Storage is required for each projector (tracks sequence position)
    let storage = svc.storage.as_ref().ok_or_else(|| {
        format!(
            "Projector '{}' missing required storage configuration",
            svc.domain
        )
    })?;
    build_storage_env(&mut env, storage);

    // Subscriber/socket name: use "name-domain" if name is set, otherwise just "domain"
    let subscriber_name = match &svc.name {
        Some(name) => format!("projector-{}-{}", name, svc.domain),
        None => format!("projector-{}", svc.domain),
    };
    let socket_name = match &svc.name {
        Some(name) => format!("{}-{}", name, svc.domain),
        None => svc.domain.clone(),
    };

    // Target configuration
    env.insert(
        "ANGZARR__TARGET__ADDRESS".to_string(),
        format!("{}/projector-{}.sock", base_path.display(), socket_name),
    );
    env.insert("ANGZARR__TARGET__DOMAIN".to_string(), svc.domain.clone());
    // Pass command as JSON array for sidecar's ManagedProcess to parse
    if !svc.command.is_empty() {
        env.insert(
            "ANGZARR__TARGET__COMMAND_JSON".to_string(),
            serde_json::to_string(&svc.command).unwrap_or_default(),
        );
    }
    if let Some(ref working_dir) = svc.working_dir {
        env.insert(
            "ANGZARR__TARGET__WORKING_DIR".to_string(),
            working_dir.clone(),
        );
    }

    // Messaging configuration - IPC or AMQP
    let listen_domain = svc.listen_domain.as_ref().unwrap_or(&svc.domain);
    if ipc_broker.is_some() {
        // IPC mode: set subscriber name and domain
        env.insert(
            "ANGZARR__MESSAGING__IPC__SUBSCRIBER_NAME".to_string(),
            subscriber_name,
        );
        env.insert(
            "ANGZARR__MESSAGING__IPC__DOMAIN".to_string(),
            listen_domain.clone(),
        );
    } else {
        // AMQP mode: set domain
        env.insert(
            "ANGZARR__MESSAGING__AMQP__DOMAIN".to_string(),
            listen_domain.clone(),
        );
    }

    // EventQuery address for repair (use listen domain's aggregate)
    // Strip routing wildcards (e.g., "game.#" -> "game") for socket paths
    let repair_domain = strip_routing_wildcards(listen_domain);
    env.insert(
        "EVENT_QUERY_ADDRESS".to_string(),
        format!("{}/aggregate-{}.sock", base_path.display(), repair_domain),
    );

    // Merge in service-specific environment variables from config
    // These override any previously set values
    for (key, value) in &svc.env {
        env.insert(key.clone(), value.clone());
    }

    Ok(env)
}

/// Build environment for stream service.
fn build_stream_env(base: &HashMap<String, String>, _config: &Config) -> HashMap<String, String> {
    // Stream service uses UDS transport from base env
    // Socket path will be {base_path}/stream.sock
    base.clone()
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

    // Set working directory
    if let Some(ref dir) = svc.working_dir {
        cmd.current_dir(dir);
    }

    // Set environment variables
    for (key, value) in &svc.env {
        cmd.env(key, value);
    }

    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    // Create new process group so we can kill all descendants
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
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let interval = std::time::Duration::from_millis(500);
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

    // Parse URL to extract host and port
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

    // Connect via TCP
    let stream = match std::net::TcpStream::connect_timeout(
        &host_port
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 80))),
        std::time::Duration::from_secs(5),
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(5)));
    let mut stream = stream;

    // Send HTTP GET request
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host_port
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    // Read response
    let mut response = [0u8; 1024];
    match stream.read(&mut response) {
        Ok(n) if n > 0 => {
            let response_str = String::from_utf8_lossy(&response[..n]);
            // Check for 2xx status code
            response_str.contains("200 ") || response_str.contains("204 ")
        }
        _ => false,
    }
}

/// Check TCP connection health.
async fn check_tcp_health(addr: &str) -> bool {
    tokio::net::TcpStream::connect(addr).await.is_ok()
}

/// Check gRPC health (simplified - just TCP connectivity).
async fn check_grpc_health(addr: &str) -> bool {
    // For now, just check TCP connectivity
    // A full implementation would use grpc.health.v1.Health
    check_tcp_health(addr).await
}

/// Build environment for gateway.
fn build_gateway_env(base: &HashMap<String, String>, config: &Config) -> HashMap<String, String> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

    // Server port (on TCP for external clients)
    let port = config.standalone.gateway.port.unwrap_or(50051);
    env.insert(
        "ANGZARR__SERVER__AGGREGATE_PORT".to_string(),
        port.to_string(),
    );

    // Use TCP transport for gateway (so external clients can connect)
    env.insert("ANGZARR__TRANSPORT__TYPE".to_string(), "tcp".to_string());
    env.insert(
        "ANGZARR__TRANSPORT__TCP__PORT".to_string(),
        port.to_string(),
    );

    // Build static endpoints from aggregates
    // Format: "domain=/path/to/socket.sock,domain2=/path/to/socket2.sock"
    let endpoints: Vec<String> = config
        .standalone
        .aggregates
        .iter()
        .map(|svc| {
            format!(
                "{}={}/aggregate-{}.sock",
                svc.domain,
                base_path.display(),
                svc.domain
            )
        })
        .collect();

    env.insert("ANGZARR_STATIC_ENDPOINTS".to_string(), endpoints.join(","));

    // Stream service address for event streaming
    env.insert(
        "STREAM_ADDRESS".to_string(),
        format!("{}/stream.sock", base_path.display()),
    );

    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_routing_wildcards() {
        // Hash wildcard (multi-level)
        assert_eq!(strip_routing_wildcards("game.#"), "game");
        assert_eq!(strip_routing_wildcards("game.player.#"), "game.player");

        // Star wildcard (single-level)
        assert_eq!(strip_routing_wildcards("game.*"), "game");
        assert_eq!(strip_routing_wildcards("game.player.*"), "game.player");

        // No wildcard - unchanged
        assert_eq!(strip_routing_wildcards("game"), "game");
        assert_eq!(strip_routing_wildcards("game.player"), "game.player");

        // Edge cases
        assert_eq!(strip_routing_wildcards("#"), "#"); // Bare # is valid match-all
        assert_eq!(strip_routing_wildcards("*"), "*"); // Bare * is valid match-all
    }
}
