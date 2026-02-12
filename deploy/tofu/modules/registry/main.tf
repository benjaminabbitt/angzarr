# Registry Module
# Aggregates service URLs from all domains and outputs discovery environment variables
#
# This module doesn't create any GCP resources - it's a data aggregation module
# that combines discovery information from all domain modules.

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

locals {
  # Combine all discovery entries
  all_services = merge(
    var.services,
    var.stream_url != null ? { "STREAM_ADDRESS" = var.stream_url } : {},
    var.topology_url != null ? { "TOPOLOGY_ADDRESS" = var.topology_url } : {},
    var.additional_env
  )

  # Extract aggregate URLs for ANGZARR_AGGREGATE_* pattern
  aggregate_urls = {
    for key, value in var.services :
    key => value if startswith(key, "ANGZARR_AGGREGATE_")
  }

  # Build JSON for projectors discovery
  projector_entries = [
    for key, value in var.services : {
      name   = lower(replace(replace(key, "ANGZARR_PROJECTOR_", ""), "_", "-"))
      domain = split("_", replace(key, "ANGZARR_PROJECTOR_", ""))[0]
      url    = value
    } if startswith(key, "ANGZARR_PROJECTOR_")
  ]
}
