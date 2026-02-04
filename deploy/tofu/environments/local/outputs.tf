# Local environment outputs

output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.angzarr.metadata[0].name
}

# Observability
output "otel_collector_endpoint" {
  description = "OTel Collector OTLP gRPC endpoint (for OTEL_EXPORTER_OTLP_ENDPOINT)"
  value       = var.enable_observability ? "angzarr-opentelemetry-collector.monitoring.svc.cluster.local:4317" : ""
}

output "grafana_url" {
  description = "Grafana URL"
  value       = var.enable_observability ? "http://localhost:3000" : ""
}

output "observability_enabled" {
  description = "Whether observability stack is enabled"
  value       = var.enable_observability
}

output "mesh_enabled" {
  description = "Whether service mesh is enabled"
  value       = var.enable_mesh
}

output "secrets_source" {
  description = "Source of credentials"
  value       = "kubernetes_secret/${var.secrets_namespace}/angzarr-secrets"
}
