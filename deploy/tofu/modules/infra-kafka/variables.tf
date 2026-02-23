# Infrastructure Module: Kafka - Variables

variable "name" {
  description = "Release name for Kafka"
  type        = string
  default     = "angzarr-mq"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "replicas" {
  description = "Number of Kafka brokers"
  type        = number
  default     = 1
}

variable "kraft_enabled" {
  description = "Use KRaft mode (no Zookeeper)"
  type        = bool
  default     = true
}

variable "security_protocol" {
  description = "Security protocol (PLAINTEXT, SSL, SASL_PLAINTEXT, SASL_SSL)"
  type        = string
  default     = "PLAINTEXT"
}

variable "persistence_enabled" {
  description = "Enable persistent storage"
  type        = bool
  default     = true
}

variable "persistence_size" {
  description = "Persistent volume size"
  type        = string
  default     = "8Gi"
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
