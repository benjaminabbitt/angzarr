# Infrastructure Module: NATS - Variables

variable "name" {
  description = "Release name for NATS"
  type        = string
  default     = "angzarr-mq"
}

variable "image" {
  description = "NATS container image"
  type        = string
  default     = "nats:2-alpine"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "jetstream_enabled" {
  description = "Enable JetStream for persistence"
  type        = bool
  default     = true
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
