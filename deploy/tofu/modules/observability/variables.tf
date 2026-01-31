# Observability module variables
# Deploys Grafana + Tempo + Prometheus + Loki + OTel Collector

variable "namespace" {
  description = "Kubernetes namespace for observability stack"
  type        = string
  default     = "monitoring"
}

variable "create_namespace" {
  description = "Create the monitoring namespace"
  type        = bool
  default     = true
}

variable "release_prefix" {
  description = "Prefix for Helm release names"
  type        = string
  default     = "angzarr"
}

# --- Grafana ---

variable "grafana_chart_version" {
  description = "Grafana Helm chart version"
  type        = string
  default     = "8.8.2"
}

variable "grafana_admin_password" {
  description = "Grafana admin password"
  type        = string
  default     = "angzarr"
  sensitive   = true
}

variable "grafana_service_type" {
  description = "Grafana service type (NodePort for kind, ClusterIP for prod)"
  type        = string
  default     = "NodePort"
}

variable "grafana_node_port" {
  description = "NodePort for Grafana (maps to host via kind extraPortMappings)"
  type        = number
  default     = 30300
}

# --- Tempo ---

variable "tempo_chart_version" {
  description = "Tempo Helm chart version"
  type        = string
  default     = "1.14.0"
}

# --- Prometheus ---

variable "prometheus_chart_version" {
  description = "Prometheus Helm chart version"
  type        = string
  default     = "27.3.1"
}

# --- Loki ---

variable "loki_chart_version" {
  description = "Loki Helm chart version"
  type        = string
  default     = "6.24.0"
}

# --- Promtail ---

variable "promtail_chart_version" {
  description = "Promtail Helm chart version"
  type        = string
  default     = "6.16.6"
}

# --- OpenTelemetry Collector ---

variable "otel_collector_chart_version" {
  description = "OpenTelemetry Collector Helm chart version"
  type        = string
  default     = "0.108.0"
}

variable "otel_collector_node_port" {
  description = "NodePort for OTel Collector gRPC (4317)"
  type        = number
  default     = 30417
}

# --- Topology ---

variable "topology_endpoint" {
  description = "Topology REST API endpoint (full URL from within cluster)"
  type        = string
  default     = ""
}

# --- Dashboards ---

variable "dashboards_path" {
  description = "Path to Grafana dashboard JSON files"
  type        = string
  default     = ""
}

# --- Resources ---

variable "resources" {
  description = "Resource requests and limits for observability components"
  type = object({
    requests = object({
      memory = string
      cpu    = string
    })
    limits = object({
      memory = string
      cpu    = string
    })
  })
  default = {
    requests = {
      memory = "128Mi"
      cpu    = "50m"
    }
    limits = {
      memory = "512Mi"
      cpu    = "500m"
    }
  }
}
