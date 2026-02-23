# Cloud Run Domain Module - Outputs (Placeholder)

output "aggregate_url" {
  description = "URL for the aggregate service"
  value       = "placeholder-not-implemented"
}

output "saga_urls" {
  description = "URLs for saga services"
  value       = {}
}

output "projector_urls" {
  description = "URLs for projector services"
  value       = {}
}

output "domain" {
  description = "Domain name"
  value       = var.domain
}
