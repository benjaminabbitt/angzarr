# SQLite Module - Outputs
# Standard interface for storage modules

locals {
  # SQLite connection URI with options
  # Format: sqlite://{path}?mode=rwc&journal_mode={mode}&synchronous={sync}&cache_size={size}
  connection_params = join("&", [
    "mode=rwc",
    "journal_mode=${var.journal_mode}",
    "synchronous=${var.synchronous}",
    "cache_size=${var.cache_size_kb}",
  ])
}

output "provides" {
  description = "Capabilities provided by this storage module"
  value = {
    capabilities  = toset(["event_store", "position_store", "snapshot_store", "transactions"])
    cloud         = null # Not cloud-specific
    rust_features = toset(["sqlite"])
    ha_mode       = "none" # SQLite is single-node
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null  # Works anywhere
    vpc             = false # No network needed
    capabilities    = null  # No dependencies
    secrets_backend = null  # No secrets needed
  }
}

output "connection_uri" {
  description = "SQLite connection URI for event store"
  value       = "sqlite://${var.data_dir}/events.db?${local.connection_params}"
}

output "event_store" {
  description = "Event store configuration for stack module"
  value = {
    connection_uri = "sqlite://${var.data_dir}/events.db?${local.connection_params}"
    provides = {
      capabilities  = toset(["event_store", "transactions"])
      rust_features = toset(["sqlite"])
    }
  }
}

output "position_store" {
  description = "Position store configuration for stack module"
  value = {
    connection_uri = "sqlite://${var.data_dir}/positions.db?${local.connection_params}"
    provides = {
      capabilities  = toset(["position_store", "transactions"])
      rust_features = toset(["sqlite"])
    }
  }
}

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = "sqlite://${var.data_dir}/snapshots.db?${local.connection_params}"
    provides = {
      capabilities  = toset(["snapshot_store"])
      rust_features = toset(["sqlite"])
    }
  }
}

output "data_dir" {
  description = "Data directory for SQLite files"
  value       = var.data_dir
}
