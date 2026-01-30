# Observability module outputs

output "otel_collector_endpoint" {
  description = "OTel Collector OTLP gRPC endpoint (for sidecar env vars)"
  value       = "${var.release_prefix}-otel-collector-opentelemetry-collector.${local.namespace}.svc.cluster.local:4317"
}

output "otel_collector_http_endpoint" {
  description = "OTel Collector OTLP HTTP endpoint"
  value       = "http://${var.release_prefix}-otel-collector-opentelemetry-collector.${local.namespace}.svc.cluster.local:4318"
}

output "grafana_url" {
  description = "Grafana URL (accessible from host via NodePort)"
  value       = "http://localhost:3000"
}

output "tempo_endpoint" {
  description = "Tempo internal endpoint"
  value       = local.tempo_endpoint
}

output "prometheus_endpoint" {
  description = "Prometheus internal endpoint"
  value       = local.prometheus_endpoint
}

output "loki_endpoint" {
  description = "Loki internal endpoint"
  value       = local.loki_endpoint
}

output "namespace" {
  description = "Monitoring namespace"
  value       = local.namespace
}
