# AWS Staging Environment - Outputs

output "cluster_name" {
  description = "ECS cluster name"
  value       = module.base.cluster_name
}

output "lb_dns_name" {
  description = "ALB DNS name for accessing services"
  value       = module.base.lb_dns_name
}

output "service_discovery_namespace" {
  description = "Cloud Map namespace for internal service discovery"
  value       = module.base.service_discovery_namespace_name
}

output "ecr_registry_url" {
  description = "ECR registry URL for pushing images"
  value       = module.ecr.registry_url
}

output "ecr_repositories" {
  description = "ECR repository URLs"
  value       = module.ecr.repository_urls
}

output "domain_services" {
  description = "Domain service information"
  value = {
    order = {
      aggregate = module.order.component_service_name
      sagas     = module.order.saga_service_names
    }
    inventory = {
      aggregate = module.inventory.component_service_name
    }
    fulfillment = {
      aggregate = module.fulfillment.component_service_name
      sagas     = module.fulfillment.saga_service_names
    }
  }
}

output "infrastructure_services" {
  description = "Infrastructure service names"
  value = {
    stream   = module.infrastructure.stream_service_name
    topology = module.infrastructure.topology_service_name
  }
}

output "discovery_env" {
  description = "Service discovery environment variables"
  value       = module.registry.discovery_env
  sensitive   = true
}
