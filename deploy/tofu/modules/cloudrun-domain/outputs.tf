# Cloud Run Domain Module - Outputs

output "aggregate_url" {
  description = "URL for the aggregate Cloud Run service"
  value       = google_cloud_run_v2_service.aggregate.uri
}

output "saga_urls" {
  description = "URLs for saga Cloud Run services"
  value = {
    for name, saga in google_cloud_run_v2_service.saga : name => saga.uri
  }
}

output "projector_urls" {
  description = "URLs for projector Cloud Run services"
  value = {
    for name, projector in google_cloud_run_v2_service.projector : name => projector.uri
  }
}

output "domain" {
  description = "Domain name"
  value       = var.domain
}

output "service_names" {
  description = "Cloud Run service names"
  value = {
    aggregate = google_cloud_run_v2_service.aggregate.name
    sagas = {
      for name, saga in google_cloud_run_v2_service.saga : name => saga.name
    }
    projectors = {
      for name, projector in google_cloud_run_v2_service.projector : name => projector.name
    }
  }
}
