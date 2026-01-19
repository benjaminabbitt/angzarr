# Redis module variables

variable "managed" {
  description = "Use cloud-managed Redis instead of Helm chart"
  type        = bool
  default     = false
}

variable "release_name" {
  description = "Helm release name"
  type        = string
  default     = "angzarr-redis"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
  default     = "angzarr"
}

variable "chart_version" {
  description = "Redis Helm chart version"
  type        = string
  default     = "20.6.0"
}

# Authentication
variable "auth_enabled" {
  description = "Enable Redis authentication"
  type        = bool
  default     = true
}

variable "password" {
  description = "Redis password (auto-generated if not provided)"
  type        = string
  default     = null
  sensitive   = true
}

# Replication
variable "replica_count" {
  description = "Number of Redis replicas (0 for standalone)"
  type        = number
  default     = 0
}

# Persistence
variable "persistence_enabled" {
  description = "Enable persistent storage"
  type        = bool
  default     = true
}

variable "persistence_size" {
  description = "Persistent volume size"
  type        = string
  default     = "2Gi"
}

# Resources
variable "resources" {
  description = "Resource requests and limits"
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
      memory = "256Mi"
      cpu    = "250m"
    }
  }
}

variable "metrics_enabled" {
  description = "Enable Prometheus metrics"
  type        = bool
  default     = true
}

# External/managed Redis connection (when managed = true)
variable "external_host" {
  description = "External Redis host (for managed Redis)"
  type        = string
  default     = ""
}

variable "external_port" {
  description = "External Redis port (for managed Redis)"
  type        = number
  default     = 6379
}

variable "external_uri" {
  description = "External Redis URI (for managed Redis)"
  type        = string
  default     = ""
  sensitive   = true
}
