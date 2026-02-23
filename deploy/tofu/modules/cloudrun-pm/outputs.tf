# Cloud Run PM Module - Outputs

output "pm_url" {
  description = "URL for the process manager Cloud Run service"
  value       = google_cloud_run_v2_service.pm.uri
}

output "name" {
  description = "Process manager name"
  value       = var.name
}

output "service_name" {
  description = "Cloud Run service name"
  value       = google_cloud_run_v2_service.pm.name
}

output "subscriptions" {
  description = "Domains this PM subscribes to"
  value       = var.subscriptions
}

output "targets" {
  description = "Domains this PM targets"
  value       = var.targets
}
