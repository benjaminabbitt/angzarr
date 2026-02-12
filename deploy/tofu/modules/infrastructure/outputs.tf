# Infrastructure Module - Outputs

output "stream_url" {
  description = "URL of the stream service"
  value       = var.stream.enabled && length(google_cloud_run_v2_service.stream) > 0 ? google_cloud_run_v2_service.stream[0].uri : null
}

output "stream_name" {
  description = "Name of the stream service"
  value       = var.stream.enabled && length(google_cloud_run_v2_service.stream) > 0 ? google_cloud_run_v2_service.stream[0].name : null
}

output "topology_url" {
  description = "URL of the topology service"
  value       = var.topology.enabled && length(google_cloud_run_v2_service.topology) > 0 ? google_cloud_run_v2_service.topology[0].uri : null
}

output "topology_name" {
  description = "Name of the topology service"
  value       = var.topology.enabled && length(google_cloud_run_v2_service.topology) > 0 ? google_cloud_run_v2_service.topology[0].name : null
}
