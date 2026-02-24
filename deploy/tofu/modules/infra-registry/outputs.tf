# Infrastructure Module: Registry - Outputs

output "service_name" {
  description = "Kubernetes service name"
  value       = local.service_name
}

output "internal_url" {
  description = "Internal cluster URL for pulling images"
  value       = "${local.service_name}.${var.namespace}.svc.cluster.local:5000"
}

output "external_url" {
  description = "External URL for pushing images (via NodePort)"
  value       = "localhost:${var.node_port}"
}

output "node_port" {
  description = "NodePort for external access"
  value       = var.node_port
}
