//! Payload store configuration.
//!
//! Configuration for payload offloading and external storage backends.

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

/// Payload store type discriminator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PayloadStoreType {
    /// Filesystem-based storage (local or mounted).
    #[default]
    Filesystem,
    /// Google Cloud Storage (requires `gcs` feature).
    #[cfg(feature = "gcs")]
    Gcs,
    /// Amazon S3 (requires `s3` feature).
    #[cfg(feature = "s3")]
    S3,
}

/// Configuration for payload offloading.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PayloadOffloadConfig {
    /// Enable payload offloading.
    /// When false, all payloads are sent inline regardless of size.
    pub enabled: bool,

    /// Payload store type.
    #[serde(rename = "type")]
    pub store_type: PayloadStoreType,

    /// Size threshold in bytes to trigger offloading.
    /// Payloads larger than this are stored externally.
    /// If 0 or not set, uses the event bus's max_message_size().
    pub threshold_bytes: usize,

    /// Retention period in hours for stored payloads.
    /// Payloads older than this may be garbage collected.
    pub retention_hours: u64,

    /// Cleanup interval in seconds for the TTL reaper.
    /// How often to scan for and remove expired payloads.
    pub cleanup_interval_secs: u64,

    /// Filesystem store configuration.
    pub filesystem: FilesystemStoreConfig,

    /// GCS store configuration (requires `gcs` feature).
    #[cfg(feature = "gcs")]
    pub gcs: GcsStoreConfig,

    /// S3 store configuration (requires `s3` feature).
    #[cfg(feature = "s3")]
    pub s3: S3StoreConfig,
}

impl Default for PayloadOffloadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            store_type: PayloadStoreType::default(),
            threshold_bytes: 0,
            retention_hours: default_retention_hours(),
            cleanup_interval_secs: default_cleanup_interval_secs(),
            filesystem: FilesystemStoreConfig::default(),
            #[cfg(feature = "gcs")]
            gcs: GcsStoreConfig::default(),
            #[cfg(feature = "s3")]
            s3: S3StoreConfig::default(),
        }
    }
}

impl PayloadOffloadConfig {
    /// Get the retention duration.
    pub fn retention(&self) -> Duration {
        Duration::from_secs(self.retention_hours * 3600)
    }

    /// Get the cleanup interval duration.
    pub fn cleanup_interval(&self) -> Duration {
        Duration::from_secs(self.cleanup_interval_secs)
    }

    /// Get the threshold, or None if should use bus default.
    pub fn threshold(&self) -> Option<usize> {
        if self.threshold_bytes > 0 {
            Some(self.threshold_bytes)
        } else {
            None
        }
    }
}

fn default_retention_hours() -> u64 {
    24
}

fn default_cleanup_interval_secs() -> u64 {
    3600 // 1 hour
}

/// Filesystem payload store configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FilesystemStoreConfig {
    /// Base directory for payload storage.
    /// Files are organized as `{base_path}/{hash[0:2]}/{hash}.bin`.
    pub base_path: PathBuf,
}

impl Default for FilesystemStoreConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("/var/angzarr/payloads"),
        }
    }
}

/// GCS payload store configuration.
#[cfg(feature = "gcs")]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GcsStoreConfig {
    /// GCS bucket name.
    pub bucket: String,
    /// Optional key prefix within the bucket.
    pub prefix: Option<String>,
}

#[cfg(feature = "gcs")]
impl Default for GcsStoreConfig {
    fn default() -> Self {
        Self {
            bucket: "angzarr-payloads".to_string(),
            prefix: None,
        }
    }
}

/// S3 payload store configuration.
#[cfg(feature = "s3")]
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct S3StoreConfig {
    /// S3 bucket name.
    pub bucket: String,
    /// Optional key prefix within the bucket.
    pub prefix: Option<String>,
    /// AWS region.
    pub region: Option<String>,
    /// Custom endpoint URL (for S3-compatible services like MinIO).
    pub endpoint: Option<String>,
}

#[cfg(feature = "s3")]
impl Default for S3StoreConfig {
    fn default() -> Self {
        Self {
            bucket: "angzarr-payloads".to_string(),
            prefix: None,
            region: None,
            endpoint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_offload_config_default() {
        let config = PayloadOffloadConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.store_type, PayloadStoreType::Filesystem);
        assert_eq!(config.threshold_bytes, 0);
        assert_eq!(config.retention_hours, 24);
        assert_eq!(config.cleanup_interval_secs, 3600);
    }

    #[test]
    fn test_payload_offload_config_threshold() {
        let mut config = PayloadOffloadConfig::default();

        // Zero threshold means use bus default
        assert_eq!(config.threshold(), None);

        // Explicit threshold
        config.threshold_bytes = 1024;
        assert_eq!(config.threshold(), Some(1024));
    }

    #[test]
    fn test_payload_offload_config_retention() {
        let config = PayloadOffloadConfig {
            retention_hours: 48,
            ..Default::default()
        };

        assert_eq!(config.retention(), Duration::from_secs(48 * 3600));
    }

    #[test]
    fn test_payload_offload_config_cleanup_interval() {
        let config = PayloadOffloadConfig {
            cleanup_interval_secs: 300,
            ..Default::default()
        };

        assert_eq!(config.cleanup_interval(), Duration::from_secs(300));
    }

    #[test]
    fn test_filesystem_store_config_default() {
        let config = FilesystemStoreConfig::default();

        assert_eq!(config.base_path, PathBuf::from("/var/angzarr/payloads"));
    }

    #[test]
    fn test_payload_offload_config_deserialize_yaml() {
        let yaml = r#"
            enabled: true
            type: filesystem
            threshold_bytes: 262144
            retention_hours: 72
            cleanup_interval_secs: 1800
            filesystem:
              base_path: /tmp/payloads
        "#;

        let config: PayloadOffloadConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(config.enabled);
        assert_eq!(config.store_type, PayloadStoreType::Filesystem);
        assert_eq!(config.threshold_bytes, 262144);
        assert_eq!(config.retention_hours, 72);
        assert_eq!(config.cleanup_interval_secs, 1800);
        assert_eq!(config.filesystem.base_path, PathBuf::from("/tmp/payloads"));
    }

    #[test]
    fn test_payload_offload_config_deserialize_minimal_yaml() {
        let yaml = r#"
            enabled: true
        "#;

        let config: PayloadOffloadConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(config.enabled);
        // All other fields should have defaults
        assert_eq!(config.store_type, PayloadStoreType::Filesystem);
        assert_eq!(config.retention_hours, 24);
    }

    #[test]
    fn test_payload_store_type_deserialize() {
        let yaml = "filesystem";
        let store_type: PayloadStoreType = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(store_type, PayloadStoreType::Filesystem);
    }
}
