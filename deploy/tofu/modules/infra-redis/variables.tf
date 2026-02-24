# Infrastructure Module: Redis - Variables

variable "name" {
  description = "Release name for Redis"
  type        = string
  default     = "angzarr-cache"
}

variable "image" {
  description = "Redis container image"
  type        = string
  default     = "redis:7"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "auth_enabled" {
  description = "Enable Redis authentication"
  type        = bool
  default     = false
}

variable "password" {
  description = "Redis password (generated if empty and auth enabled)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "persistence_enabled" {
  description = "Enable persistent storage (not yet implemented)"
  type        = bool
  default     = false
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
