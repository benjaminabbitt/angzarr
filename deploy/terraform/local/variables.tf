variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/config"
}

variable "kubeconfig_context" {
  description = "Kubernetes context to use"
  type        = string
  default     = ""
}

variable "namespace" {
  description = "Kubernetes namespace for evented"
  type        = string
  default     = "evented"
}

variable "replicas" {
  description = "Number of replicas per application"
  type        = number
  default     = 1
}

variable "evented_image_repository" {
  description = "Evented sidecar Docker image repository"
  type        = string
  default     = "evented"
}

variable "evented_image_tag" {
  description = "Evented sidecar Docker image tag"
  type        = string
  default     = "latest"
}

variable "log_level" {
  description = "Log level (trace, debug, info, warn, error)"
  type        = string
  default     = "info"
}

variable "enable_rabbitmq" {
  description = "Deploy RabbitMQ"
  type        = bool
  default     = true
}

variable "rabbitmq_user" {
  description = "RabbitMQ username"
  type        = string
}

variable "rabbitmq_password" {
  description = "RabbitMQ password"
  type        = string
  sensitive   = true
}

variable "rabbitmq_chart_version" {
  description = "RabbitMQ Helm chart version"
  type        = string
  default     = "15.0.0"
}

variable "enable_redis" {
  description = "Deploy Redis for event store"
  type        = bool
  default     = false
}

variable "redis_chart_version" {
  description = "Redis Helm chart version"
  type        = string
  default     = "20.0.0"
}

variable "storage_type" {
  description = "Storage backend: sqlite or redis"
  type        = string
  default     = "sqlite"
}

# Application configurations
variable "business_applications" {
  description = "List of business logic applications (evented runs as sidecar)"
  type = list(object({
    name   = string
    domain = string
    port   = number
    image = object({
      repository = string
      tag        = string
    })
    type = string
    python = optional(object({
      module = string
      path   = string
    }))
    go = optional(object({
      library = string
    }))
  }))
  default = []
}

variable "projector_applications" {
  description = "List of projector applications (evented runs as sidecar)"
  type = list(object({
    name   = string
    topics = list(string)
    image = object({
      repository = string
      tag        = string
    })
    type = string
    python = optional(object({
      module = string
      path   = string
    }))
    go = optional(object({
      library = string
    }))
  }))
  default = []
}

variable "saga_applications" {
  description = "List of saga applications (evented runs as sidecar)"
  type = list(object({
    name   = string
    topics = list(string)
    image = object({
      repository = string
      tag        = string
    })
    type = string
    python = optional(object({
      module = string
      path   = string
    }))
    go = optional(object({
      library = string
    }))
  }))
  default = []
}
