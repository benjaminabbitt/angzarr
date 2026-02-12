# GCP Environment - Outputs

output "database_connection" {
  description = "Cloud SQL connection name"
  value       = module.cloudsql.connection_name
  sensitive   = true
}

output "events_topic" {
  description = "Pub/Sub events topic name"
  value       = module.pubsub.events_topic_name
}

output "order_aggregate_url" {
  description = "Order aggregate service URL"
  value       = module.order.component_url
}

output "inventory_aggregate_url" {
  description = "Inventory aggregate service URL"
  value       = module.inventory.component_url
}

output "fulfillment_aggregate_url" {
  description = "Fulfillment aggregate service URL"
  value       = module.fulfillment.component_url
}

output "stream_url" {
  description = "Stream service URL"
  value       = module.infrastructure.stream_url
}

output "topology_url" {
  description = "Topology service URL"
  value       = module.infrastructure.topology_url
}

output "discovery_env" {
  description = "Service discovery environment variables"
  value       = module.registry.discovery_env
}
