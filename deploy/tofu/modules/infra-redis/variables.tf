# Infrastructure Module: Redis - Variables

variable "name" {
  description = "Release name for Redis"
  type        = string
  default     = "angzarr-cache"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "chart_version" {
  description = "Bitnami Redis chart version"
  type        = string
  default     = "19.0.0"
}

variable "auth_enabled" {
  description = "Enable Redis authentication"
  type        = bool
  default     = true
}

variable "password" {
  description = "Redis password (generated if empty)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "replica_count" {
  description = "Number of Redis replicas"
  type        = number
  default     = 0
}

variable "persistence_enabled" {
  description = "Enable persistent storage"
  type        = bool
  default     = true
}

variable "persistence_size" {
  description = "Persistent volume size"
  type        = string
  default     = "1Gi"
}

variable "storage_class" {
  description = "Storage class for PVC"
  type        = string
  default     = ""
}

variable "resources" {
  description = "Resource limits and requests"
  type = object({
    limits = optional(object({
      cpu    = optional(string, "500m")
      memory = optional(string, "512Mi")
    }), {})
    requests = optional(object({
      cpu    = optional(string, "100m")
      memory = optional(string, "256Mi")
    }), {})
  })
  default = {}
}
