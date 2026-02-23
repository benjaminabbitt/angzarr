# Infrastructure Module: RabbitMQ - Variables

variable "name" {
  description = "Release name for RabbitMQ"
  type        = string
  default     = "angzarr-mq"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "username" {
  description = "RabbitMQ username"
  type        = string
  default     = "angzarr"
}

variable "password" {
  description = "RabbitMQ password (generated if empty)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "erlang_cookie" {
  description = "Erlang cookie for clustering (generated if empty)"
  type        = string
  default     = ""
  sensitive   = true
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
