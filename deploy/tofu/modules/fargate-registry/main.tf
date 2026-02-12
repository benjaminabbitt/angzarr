# Fargate Registry Module
# Aggregates service discovery entries from all domains
# AWS equivalent of the GCP registry module
#
# This module doesn't create any AWS resources - it's a data aggregation module
# that combines discovery information from all domain modules.

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

locals {
  # Build full DNS names if namespace provided
  stream_address = var.stream_dns != null && var.namespace_name != null ? (
    "${var.stream_dns}.${var.namespace_name}:1340"
  ) : var.stream_dns

  topology_address = var.topology_dns != null && var.namespace_name != null ? (
    "${var.topology_dns}.${var.namespace_name}:9099"
  ) : var.topology_dns

  # Combine all discovery entries
  all_services = merge(
    var.services,
    local.stream_address != null ? { "STREAM_ADDRESS" = local.stream_address } : {},
    local.topology_address != null ? { "TOPOLOGY_ADDRESS" = local.topology_address } : {},
    var.additional_env
  )

  # Extract aggregate addresses for ANGZARR_AGGREGATE_* pattern
  aggregate_addresses = {
    for key, value in var.services :
    key => value if startswith(key, "ANGZARR_AGGREGATE_")
  }

  # Build JSON for projectors discovery
  projector_entries = [
    for key, value in var.services : {
      name   = lower(replace(replace(key, "ANGZARR_PROJECTOR_", ""), "_", "-"))
      domain = split("_", replace(key, "ANGZARR_PROJECTOR_", ""))[0]
      dns    = value
    } if startswith(key, "ANGZARR_PROJECTOR_")
  ]
}
