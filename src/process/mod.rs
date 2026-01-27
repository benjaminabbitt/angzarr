//! Process management for spawning business logic services.
//!
//! Handles spawning child processes for business logic, passing configuration
//! as environment variables, and managing their lifecycle.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::sleep;
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};

use crate::transport::{connect_to_address, TransportConfig};

/// Environment variables passed to spawned processes.
pub struct ProcessEnv {
    /// Transport type ("tcp" or "uds").
    pub transport_type: String,
    /// For UDS: base path for sockets.
    pub uds_base_path: Option<String>,
    /// Service name for socket naming (e.g., "business", "saga", "projector").
    pub service_name: String,
    /// Domain or handler name for socket qualification.
    pub domain: Option<String>,
    /// TCP port (if using TCP transport).
    pub port: Option<u16>,
}

impl ProcessEnv {
    /// Create environment variables from transport config.
    pub fn from_transport(
        transport: &TransportConfig,
        service_name: &str,
        domain: Option<&str>,
    ) -> Self {
        use crate::transport::TransportType;

        let transport_type = match transport.transport_type {
            TransportType::Tcp => "tcp".to_string(),
            TransportType::Uds => "uds".to_string(),
        };

        Self {
            transport_type,
            uds_base_path: Some(transport.uds.base_path.to_string_lossy().to_string()),
            service_name: service_name.to_string(),
            domain: domain.map(|s| s.to_string()),
            port: None,
        }
    }

    /// Convert to environment variable map.
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("TRANSPORT_TYPE".to_string(), self.transport_type.clone());

        if let Some(ref base_path) = self.uds_base_path {
            env.insert("UDS_BASE_PATH".to_string(), base_path.clone());
        }

        env.insert("SERVICE_NAME".to_string(), self.service_name.clone());

        if let Some(ref domain) = self.domain {
            env.insert("DOMAIN".to_string(), domain.clone());
        }

        if let Some(port) = self.port {
            env.insert("PORT".to_string(), port.to_string());
        }

        env
    }
}

/// Manages a spawned child process.
pub struct ManagedProcess {
    child: Child,
    command: String,
}

impl ManagedProcess {
    /// Spawn a new process with the given command array and environment.
    ///
    /// Command is an array where the first element is the executable and
    /// the rest are arguments. No shell interpretation - direct exec.
    ///
    /// Example: `["python", "-m", "myapp", "--port", "8080"]`
    pub async fn spawn(
        command: &[String],
        working_dir: Option<&str>,
        env: &ProcessEnv,
        extra_env: Option<&HashMap<String, String>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if command.is_empty() {
            return Err("Command array cannot be empty".into());
        }

        let executable = &command[0];
        let args = &command[1..];

        info!(executable = %executable, ?args, "Spawning business logic process");

        let env_vars = env.to_env_vars();
        debug!(?env_vars, "Process environment");

        let mut cmd = Command::new(executable);
        cmd.args(args);

        // Set working directory if specified
        if let Some(dir) = working_dir {
            let path = Path::new(dir);
            if path.exists() {
                cmd.current_dir(path);
            } else {
                warn!(dir = %dir, "Working directory does not exist, using current directory");
            }
        }

        // Set environment variables from ProcessEnv
        for (key, value) in env_vars {
            cmd.env(&key, &value);
        }

        // Set extra environment variables from config
        if let Some(extra) = extra_env {
            for (key, value) in extra {
                cmd.env(key, value);
            }
        }

        // Redirect output to inherit (visible in sidecar logs)
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let child = cmd.spawn().map_err(|e| {
            error!(executable = %executable, error = %e, "Failed to spawn process");
            e
        })?;

        info!(pid = ?child.id(), "Business logic process spawned");

        Ok(Self {
            child,
            command: command.join(" "),
        })
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(None) => true, // Still running
            Ok(Some(status)) => {
                warn!(status = ?status, command = %self.command, "Process exited");
                false
            }
            Err(e) => {
                error!(error = %e, "Failed to check process status");
                false
            }
        }
    }

    /// Kill the process.
    pub async fn kill(&mut self) -> Result<(), std::io::Error> {
        info!(pid = ?self.child.id(), "Killing business logic process");
        self.child.kill().await
    }
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        // Try to kill the process on drop
        if let Ok(None) = self.child.try_wait() {
            warn!(pid = ?self.child.id(), "Killing orphaned process on drop");
            // Use start_kill for non-async drop
            let _ = self.child.start_kill();
        }
    }
}

/// Wait for a service to become ready by attempting to connect.
pub async fn wait_for_ready(
    address: &str,
    timeout: Duration,
    retry_interval: Duration,
) -> Result<Channel, Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    loop {
        match connect_to_address(address).await {
            Ok(channel) => {
                info!(address = %address, "Service is ready");
                return Ok(channel);
            }
            Err(e) => {
                if start.elapsed() > timeout {
                    error!(address = %address, error = %e, "Timeout waiting for service");
                    return Err(format!("Timeout waiting for service at {}: {}", address, e).into());
                }
                debug!(address = %address, "Service not ready, retrying...");
                sleep(retry_interval).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_env_to_env_vars() {
        let env = ProcessEnv {
            transport_type: "uds".to_string(),
            uds_base_path: Some("/tmp/angzarr".to_string()),
            service_name: "business".to_string(),
            domain: Some("customer".to_string()),
            port: None,
        };

        let vars = env.to_env_vars();
        assert_eq!(vars.get("TRANSPORT_TYPE"), Some(&"uds".to_string()));
        assert_eq!(vars.get("UDS_BASE_PATH"), Some(&"/tmp/angzarr".to_string()));
        assert_eq!(vars.get("SERVICE_NAME"), Some(&"business".to_string()));
        assert_eq!(vars.get("DOMAIN"), Some(&"customer".to_string()));
    }

    #[test]
    fn test_process_env_tcp() {
        let env = ProcessEnv {
            transport_type: "tcp".to_string(),
            uds_base_path: None,
            service_name: "business".to_string(),
            domain: Some("order".to_string()),
            port: Some(50051),
        };

        let vars = env.to_env_vars();
        assert_eq!(vars.get("TRANSPORT_TYPE"), Some(&"tcp".to_string()));
        assert_eq!(vars.get("PORT"), Some(&"50051".to_string()));
        assert!(vars.get("UDS_BASE_PATH").is_none());
    }
}
