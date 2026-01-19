# Messaging module variables

variable "type" {
  description = "Message broker type: rabbitmq or kafka"
  type        = string
  validation {
    condition     = contains(["rabbitmq", "kafka"], var.type)
    error_message = "Messaging type must be 'rabbitmq' or 'kafka'."
  }
}

variable "managed" {
  description = "Use cloud-managed message broker instead of Helm chart"
  type        = bool
  default     = false
}

variable "release_name" {
  description = "Helm release name"
  type        = string
  default     = "angzarr-mq"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
  default     = "angzarr"
}

variable "rabbitmq_chart_version" {
  description = "RabbitMQ Helm chart version"
  type        = string
  default     = "16.0.14"
}

variable "kafka_chart_version" {
  description = "Kafka Helm chart version"
  type        = string
  default     = "32.0.0"
}

# Authentication
variable "username" {
  description = "Message broker username"
  type        = string
  default     = "angzarr"
}

variable "password" {
  description = "Message broker password (auto-generated if not provided)"
  type        = string
  default     = null
  sensitive   = true
}

variable "kafka_sasl_enabled" {
  description = "Enable SASL authentication for Kafka"
  type        = bool
  default     = false
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
  default     = "8Gi"
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
      memory = "256Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "1Gi"
      cpu    = "500m"
    }
  }
}

variable "metrics_enabled" {
  description = "Enable Prometheus metrics"
  type        = bool
  default     = true
}

# External/managed broker connection (when managed = true)
variable "external_host" {
  description = "External broker host (for managed brokers)"
  type        = string
  default     = ""
}

variable "external_port" {
  description = "External broker port (for managed brokers)"
  type        = number
  default     = 0
}

variable "external_uri" {
  description = "External broker URI (for managed brokers)"
  type        = string
  default     = ""
  sensitive   = true
}
