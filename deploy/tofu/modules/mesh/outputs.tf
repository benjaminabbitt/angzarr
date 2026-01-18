# Service Mesh module outputs

output "type" {
  description = "Service mesh type"
  value       = var.type
}

output "namespace" {
  description = "Mesh control plane namespace"
  value       = var.type == "linkerd" ? "linkerd" : "istio-system"
}

output "injection_annotation" {
  description = "Annotation to add to namespaces for automatic injection"
  value = var.type == "linkerd" ? {
    key   = "linkerd.io/inject"
    value = "enabled"
    } : {
    key   = "istio-injection"
    value = "enabled"
  }
}

output "angzarr_namespace" {
  description = "Angzarr namespace (if created)"
  value       = var.inject_namespace ? var.namespace : null
}
