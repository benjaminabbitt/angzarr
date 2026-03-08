//! Unix Domain Socket helpers.

use std::path::{Path, PathBuf};

use tracing::info;

/// RAII guard for cleaning up UDS socket files.
pub struct UdsCleanupGuard {
    path: PathBuf,
}

impl UdsCleanupGuard {
    /// Create a new cleanup guard for the given socket path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the socket path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UdsCleanupGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(e) = std::fs::remove_file(&self.path) {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "Failed to clean up UDS socket"
                );
            } else {
                tracing::debug!(
                    path = %self.path.display(),
                    "Cleaned up UDS socket"
                );
            }
        }
    }
}

/// Prepare a UDS socket path for binding.
///
/// - Creates parent directories if needed (mode 0700 for security)
/// - Removes stale socket file if exists
/// - Returns a cleanup guard that removes the socket on drop
pub fn prepare_uds_socket(path: &Path) -> std::io::Result<UdsCleanupGuard> {
    // Create parent directories with restrictive permissions
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        // Set directory permissions to owner-only (0700)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(parent, perms)?;
        }
    }

    // Remove stale socket file
    if path.exists() {
        info!(path = %path.display(), "Removing stale UDS socket");
        std::fs::remove_file(path)?;
    }

    Ok(UdsCleanupGuard::new(path))
}
