# K8s Domain Module - Outputs

output "aggregate_url" {
  description = "Internal service URL for the aggregate"
  value       = "${var.domain}-aggregate.${var.namespace}.svc.cluster.local:1310"
}

output "saga_urls" {
  description = "Internal service URLs for sagas"
  value = {
    for name, _ in var.sagas :
    name => "saga-${var.domain}-${name}.${var.namespace}.svc.cluster.local:1310"
  }
}

output "projector_urls" {
  description = "Internal service URLs for projectors"
  value = {
    for name, _ in var.projectors :
    name => "projector-${var.domain}-${name}.${var.namespace}.svc.cluster.local:1310"
  }
}

output "domain" {
  description = "Domain name"
  value       = var.domain
}
