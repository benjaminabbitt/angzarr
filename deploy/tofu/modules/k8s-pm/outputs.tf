# K8s PM Module - Outputs

output "pm_url" {
  description = "Internal service URL for the process manager"
  value       = "pm-${var.name}.${var.namespace}.svc.cluster.local:1310"
}

output "name" {
  description = "Process manager name"
  value       = var.name
}
