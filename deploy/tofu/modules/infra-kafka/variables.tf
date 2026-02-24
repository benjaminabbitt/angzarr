# Infrastructure Module: Kafka - Variables

variable "name" {
  description = "Release name for Kafka"
  type        = string
  default     = "angzarr-mq"
}

variable "image" {
  description = "Kafka container image"
  type        = string
  default     = "apache/kafka:3.7.0"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "resources" {
  description = "Resource limits and requests"
  type = object({
    limits = optional(object({
      cpu    = optional(string, "1")
      memory = optional(string, "2Gi")
    }), {})
    requests = optional(object({
      cpu    = optional(string, "250m")
      memory = optional(string, "512Mi")
    }), {})
  })
  default = {}
}
