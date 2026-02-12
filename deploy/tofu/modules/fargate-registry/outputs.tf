# Fargate Registry Module - Outputs

output "discovery_env" {
  description = "Environment variables for service discovery (pass to coordinator_env)"
  value       = local.all_services
}

output "aggregate_addresses" {
  description = "Map of aggregate addresses only (ANGZARR_AGGREGATE_*)"
  value       = local.aggregate_addresses
}

output "projectors_json" {
  description = "JSON array of projector entries for ANGZARR_PROJECTORS env var"
  value       = jsonencode(local.projector_entries)
}

output "all_services" {
  description = "All registered service addresses"
  value       = local.all_services
}
