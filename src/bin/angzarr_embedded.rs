//! angzarr-embedded: Embedded mode orchestrator
//!
//! Spawns and manages all sidecar processes for local development.
//! Replaces K8s orchestration with a single binary.
//!
//! ## Architecture
//! ```text
//! angzarr-embedded (orchestrator)
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
//! embedded:
//!   aggregates:
//!     - domain: customer
//!       command: uv run --directory customer python server.py
//!     - domain: order
//!       command: uv run --directory order python server.py
//!   sagas:
//!     - domain: fulfillment
//!       listen_domains: [order]
//!       command: uv run --directory saga-fulfillment python server.py
//!   projectors:
//!     - domain: web
//!       command: uv run --directory projector-web python server.py
//!   gateway:
//!     enabled: true
//!     port: 50051
//! ```

use std::collections::HashMap;
use std::process::Stdio;

use tokio::process::{Child, Command};
use tracing::{error, info, warn};

use angzarr::bus::{IpcBroker, IpcBrokerConfig, MessagingType, SUBSCRIBERS_ENV_VAR};
use angzarr::config::{Config, ServiceConfig};

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

        let mut cmd = Command::new(binary);
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
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                self.child.wait(),
            )
            .await
            {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    angzarr::utils::bootstrap::init_tracing();

    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    info!("Starting angzarr-embedded orchestrator");

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
    let bin_dir = self_path.parent().unwrap_or_else(|| std::path::Path::new("."));

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
    for svc in &config.embedded.aggregates {
        let mut env = build_aggregate_env(&base_env, &config, svc);
        // Pass subscriber list for IPC fanout
        if ipc_broker.is_some() {
            env.insert(SUBSCRIBERS_ENV_VAR.to_string(), subscribers_json.clone());
        }
        let name = format!("aggregate-{}", svc.domain);
        let child = ManagedChild::spawn(&name, &aggregate_bin, env).await?;
        children.push(child);
    }

    // Spawn saga sidecars
    for svc in &config.embedded.sagas {
        let env = build_saga_env(&base_env, &config, svc, &ipc_broker);
        let name = format!("saga-{}", svc.domain);
        let child = ManagedChild::spawn(&name, &saga_bin, env).await?;
        children.push(child);
    }

    // Spawn projector sidecars
    for svc in &config.embedded.projectors {
        let env = build_projector_env(&base_env, &config, svc, &ipc_broker);
        let name = match &svc.name {
            Some(proj_name) => format!("projector-{}-{}", proj_name, svc.domain),
            None => format!("projector-{}", svc.domain),
        };
        let child = ManagedChild::spawn(&name, &projector_bin, env).await?;
        children.push(child);
    }

    // Spawn stream service and its projector sidecar if gateway is enabled
    // Stream enables execute_stream() for real-time event streaming to clients
    if config.embedded.gateway.enabled {
        // Spawn angzarr-stream (receives events, provides subscriptions)
        let stream_env = build_stream_env(&base_env, &config);
        let child = ManagedChild::spawn("stream", &stream_bin, stream_env).await?;
        children.push(child);

        // Spawn projector sidecar to feed events to stream service
        // This sidecar subscribes to all events and forwards to angzarr-stream
        let stream_projector_env = build_stream_projector_env(&base_env, &config, &ipc_broker);
        let child = ManagedChild::spawn("projector-stream", &projector_bin, stream_projector_env).await?;
        children.push(child);
    }

    // Spawn gateway if enabled
    if config.embedded.gateway.enabled {
        let env = build_gateway_env(&base_env, &config);
        let child = ManagedChild::spawn("gateway", &gateway_bin, env).await?;
        children.push(child);
    }

    info!(
        aggregates = config.embedded.aggregates.len(),
        sagas = config.embedded.sagas.len(),
        projectors = config.embedded.projectors.len(),
        gateway = config.embedded.gateway.enabled,
        streaming = config.embedded.gateway.enabled,
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
fn find_binary(bin_dir: &std::path::Path, name: &str) -> Result<String, Box<dyn std::error::Error>> {
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

/// Build base environment variables from config.
fn build_base_env(config: &Config) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Storage
    env.insert(
        "ANGZARR__STORAGE__TYPE".to_string(),
        format!("{:?}", config.storage.storage_type).to_lowercase(),
    );

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

/// Build environment for aggregate sidecar.
fn build_aggregate_env(
    base: &HashMap<String, String>,
    config: &Config,
    svc: &ServiceConfig,
) -> HashMap<String, String> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

    // Target configuration
    env.insert(
        "ANGZARR__TARGET__ADDRESS".to_string(),
        format!("{}/business-{}.sock", base_path.display(), svc.domain),
    );
    env.insert("ANGZARR__TARGET__DOMAIN".to_string(), svc.domain.clone());
    env.insert("ANGZARR__TARGET__COMMAND".to_string(), svc.command.clone());

    env
}

/// Set up IPC broker if using IPC messaging.
async fn setup_ipc_broker(config: &Config) -> Result<Option<IpcBroker>, Box<dyn std::error::Error>> {
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
    for svc in &config.embedded.projectors {
        let name = match &svc.name {
            Some(proj_name) => format!("projector-{}-{}", proj_name, svc.domain),
            None => format!("projector-{}", svc.domain),
        };
        let domains = if svc.listen_domains.is_empty() {
            vec![svc.domain.clone()]
        } else {
            svc.listen_domains.clone()
        };
        broker.register_subscriber(&name, domains)?;
    }

    // Register all sagas as subscribers
    for svc in &config.embedded.sagas {
        let name = format!("saga-{}", svc.domain);
        let domains = if svc.listen_domains.is_empty() {
            vec![svc.domain.clone()]
        } else {
            svc.listen_domains.clone()
        };
        broker.register_subscriber(&name, domains)?;
    }

    // Register stream projector if gateway enabled (subscribes to all)
    if config.embedded.gateway.enabled {
        broker.register_subscriber("projector-stream", vec!["#".to_string()])?;
    }

    info!(subscribers = broker.get_subscribers().len(), "IPC broker ready");

    Ok(Some(broker))
}

/// Build environment for saga sidecar.
fn build_saga_env(
    base: &HashMap<String, String>,
    config: &Config,
    svc: &ServiceConfig,
    ipc_broker: &Option<IpcBroker>,
) -> HashMap<String, String> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;
    let subscriber_name = format!("saga-{}", svc.domain);

    // Target configuration
    env.insert(
        "ANGZARR__TARGET__ADDRESS".to_string(),
        format!("{}/saga-{}.sock", base_path.display(), svc.domain),
    );
    env.insert("ANGZARR__TARGET__DOMAIN".to_string(), svc.domain.clone());
    env.insert("ANGZARR__TARGET__COMMAND".to_string(), svc.command.clone());

    // Messaging configuration - IPC or AMQP
    if ipc_broker.is_some() {
        // IPC mode: set subscriber name and domain (singular, comma-separated)
        env.insert(
            "ANGZARR__MESSAGING__IPC__SUBSCRIBER_NAME".to_string(),
            subscriber_name,
        );
        if !svc.listen_domains.is_empty() {
            env.insert(
                "ANGZARR__MESSAGING__IPC__DOMAIN".to_string(),
                svc.listen_domains.join(","),
            );
        }
    } else if !svc.listen_domains.is_empty() {
        // AMQP mode: set domain (singular, first domain wins for AMQP)
        env.insert(
            "ANGZARR__MESSAGING__AMQP__DOMAIN".to_string(),
            svc.listen_domains.first().cloned().unwrap_or_default(),
        );
    }

    // Command handler address (use gateway or first aggregate)
    if config.embedded.gateway.enabled {
        let port = config.embedded.gateway.port.unwrap_or(50051);
        env.insert("COMMAND_ADDRESS".to_string(), format!("localhost:{}", port));
    } else if let Some(first_agg) = config.embedded.aggregates.first() {
        env.insert(
            "COMMAND_ADDRESS".to_string(),
            format!("{}/aggregate-{}.sock", base_path.display(), first_agg.domain),
        );
    }

    env
}

/// Build environment for projector sidecar.
fn build_projector_env(
    base: &HashMap<String, String>,
    config: &Config,
    svc: &ServiceConfig,
    ipc_broker: &Option<IpcBroker>,
) -> HashMap<String, String> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

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
    env.insert("ANGZARR__TARGET__COMMAND".to_string(), svc.command.clone());

    // Messaging configuration - IPC or AMQP
    if ipc_broker.is_some() {
        // IPC mode: set subscriber name and domain (singular, comma-separated)
        env.insert(
            "ANGZARR__MESSAGING__IPC__SUBSCRIBER_NAME".to_string(),
            subscriber_name,
        );
        let domains = if svc.listen_domains.is_empty() {
            vec![svc.domain.clone()]
        } else {
            svc.listen_domains.clone()
        };
        env.insert(
            "ANGZARR__MESSAGING__IPC__DOMAIN".to_string(),
            domains.join(","),
        );
    } else if !svc.listen_domains.is_empty() {
        // AMQP mode: set domain (singular)
        env.insert(
            "ANGZARR__MESSAGING__AMQP__DOMAIN".to_string(),
            svc.listen_domains.first().cloned().unwrap_or_default(),
        );
    }

    env
}

/// Build environment for stream service.
fn build_stream_env(base: &HashMap<String, String>, _config: &Config) -> HashMap<String, String> {
    // Stream service uses UDS transport from base env
    // Socket path will be {base_path}/stream.sock
    base.clone()
}

/// Build environment for the projector sidecar that feeds events to stream service.
fn build_stream_projector_env(
    base: &HashMap<String, String>,
    config: &Config,
    ipc_broker: &Option<IpcBroker>,
) -> HashMap<String, String> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

    // Target is the stream service socket
    env.insert(
        "ANGZARR__TARGET__ADDRESS".to_string(),
        format!("{}/stream.sock", base_path.display()),
    );
    env.insert("ANGZARR__TARGET__DOMAIN".to_string(), "stream".to_string());
    // No TARGET__COMMAND - stream service is already running

    // Messaging configuration - IPC or AMQP
    if ipc_broker.is_some() {
        // IPC mode: set subscriber name and subscribe to all domains
        env.insert(
            "ANGZARR__MESSAGING__IPC__SUBSCRIBER_NAME".to_string(),
            "projector-stream".to_string(),
        );
        env.insert(
            "ANGZARR__MESSAGING__IPC__DOMAIN".to_string(),
            "#".to_string(),
        );
    } else {
        // AMQP mode: subscribe to all domains
        env.insert(
            "ANGZARR__MESSAGING__AMQP__DOMAIN".to_string(),
            "#".to_string(),
        );
    }

    env
}

/// Build environment for gateway.
fn build_gateway_env(base: &HashMap<String, String>, config: &Config) -> HashMap<String, String> {
    let mut env = base.clone();
    let base_path = &config.transport.uds.base_path;

    // Server port (on TCP for external clients)
    let port = config.embedded.gateway.port.unwrap_or(50051);
    env.insert("ANGZARR__SERVER__AGGREGATE_PORT".to_string(), port.to_string());

    // Use TCP transport for gateway (so external clients can connect)
    env.insert("ANGZARR__TRANSPORT__TYPE".to_string(), "tcp".to_string());
    env.insert("ANGZARR__TRANSPORT__TCP__PORT".to_string(), port.to_string());

    // Build static endpoints from aggregates
    // Format: "domain=/path/to/socket.sock,domain2=/path/to/socket2.sock"
    let endpoints: Vec<String> = config
        .embedded
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
