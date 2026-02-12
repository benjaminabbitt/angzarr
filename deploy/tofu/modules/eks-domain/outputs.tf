# EKS Domain Module - Outputs

output "release_name" {
  description = "Helm release name"
  value       = helm_release.domain.name
}

output "release_namespace" {
  description = "Kubernetes namespace"
  value       = helm_release.domain.namespace
}

output "release_status" {
  description = "Helm release status"
  value       = helm_release.domain.status
}

output "release_revision" {
  description = "Helm release revision"
  value       = helm_release.domain.version
}

# Service discovery - construct DNS names based on Helm naming conventions
output "aggregate_service_dns" {
  description = "Aggregate service DNS name"
  value       = var.aggregate.enabled ? "${var.domain}-aggregate.${var.namespace}.svc.cluster.local" : null
}

output "process_manager_service_dns" {
  description = "Process manager service DNS name"
  value       = var.process_manager.enabled ? "${var.domain}-pm.${var.namespace}.svc.cluster.local" : null
}

output "saga_service_dns" {
  description = "Map of saga name to service DNS name"
  value = {
    for name, _ in var.sagas :
    name => "saga-${var.domain}-${name}.${var.namespace}.svc.cluster.local"
  }
}

output "projector_service_dns" {
  description = "Map of projector name to service DNS name"
  value = {
    for name, _ in var.projectors :
    name => "projector-${var.domain}-${name}.${var.namespace}.svc.cluster.local"
  }
}

# Discovery entries for environment variable injection
output "discovery_entries" {
  description = "Service discovery environment variables"
  value = merge(
    var.aggregate.enabled ? {
      "ANGZARR_AGGREGATE_${upper(var.domain)}" = "${var.domain}-aggregate.${var.namespace}.svc.cluster.local:1310"
    } : {},
    var.process_manager.enabled ? {
      "ANGZARR_PM_${upper(var.domain)}" = "${var.domain}-pm.${var.namespace}.svc.cluster.local:1310"
    } : {},
    {
      for name, _ in var.sagas :
      "ANGZARR_SAGA_${upper(var.domain)}_${upper(name)}" => "saga-${var.domain}-${name}.${var.namespace}.svc.cluster.local:1310"
    },
    {
      for name, _ in var.projectors :
      "ANGZARR_PROJECTOR_${upper(var.domain)}_${upper(name)}" => "projector-${var.domain}-${name}.${var.namespace}.svc.cluster.local:1310"
    }
  )
}
