# Bigtable Module - Outputs

locals {
  connection_uri = "bigtable://${var.project_id}/${var.instance_name}"
}

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities    = toset(["event_store", "position_store", "snapshot_store"])
    cloud           = "gcp"
    rust_features   = toset(["bigtable"])
    ha_mode         = "single-az" # Single cluster, can add replication for HA
    secrets_backend = "gcp"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null
    vpc             = false # Uses Google APIs
    capabilities    = null
    secrets_backend = null
  }
}

output "connection_uri" {
  description = "Connection URI for coordinators"
  value       = local.connection_uri
}

output "event_store" {
  description = "Event store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["event_store"])
      rust_features = toset(["bigtable"])
    }
  }
}

output "position_store" {
  description = "Position store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["position_store"])
      rust_features = toset(["bigtable"])
    }
  }
}

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["snapshot_store"])
      rust_features = toset(["bigtable"])
    }
  }
}

output "instance_name" {
  description = "Bigtable instance name"
  value       = google_bigtable_instance.angzarr.name
}

output "instance_id" {
  description = "Bigtable instance ID"
  value       = google_bigtable_instance.angzarr.id
}

output "table_names" {
  description = "Map of table purposes to table names"
  value = {
    events    = google_bigtable_table.events.name
    positions = google_bigtable_table.positions.name
    snapshots = google_bigtable_table.snapshots.name
  }
}
