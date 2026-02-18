# Operational Configuration (Kubernetes/Helm)
# Defines HOW the domain runs - uses angzarr Helm chart

#------------------------------------------------------------------------------
# Namespace
#------------------------------------------------------------------------------
variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "create_namespace" {
  description = "Create namespace if it doesn't exist"
  type        = bool
  default     = false
}

#------------------------------------------------------------------------------
# Helm Chart
#------------------------------------------------------------------------------
variable "chart_path" {
  description = "Path to local Helm chart (overrides chart_repository)"
  type        = string
  default     = null
}

variable "chart_repository" {
  description = "Helm chart repository URL"
  type        = string
  default     = null
}

variable "wait" {
  description = "Wait for resources to be ready"
  type        = bool
  default     = true
}

variable "timeout" {
  description = "Helm install/upgrade timeout in seconds"
  type        = number
  default     = 300
}

#------------------------------------------------------------------------------
# Images
#------------------------------------------------------------------------------
variable "images" {
  description = "Container images for all components"
  type = object({
    coordinator_aggregate = string
    coordinator_saga      = string
    coordinator_projector = string
    coordinator_pm        = optional(string)
    logic                 = string
    upcaster              = optional(string)
    saga_logic            = optional(map(string), {})
    projector_logic       = optional(map(string), {})
  })
}

#------------------------------------------------------------------------------
# Scaling
#------------------------------------------------------------------------------
variable "scaling" {
  description = "Per-component scaling configuration"
  type = object({
    aggregate = optional(object({
      replicas     = optional(number, 1)
      min_replicas = optional(number, 1)
      max_replicas = optional(number, 10)
      resources = optional(object({
        requests = optional(object({
          cpu    = optional(string, "100m")
          memory = optional(string, "128Mi")
        }), {})
        limits = optional(object({
          cpu    = optional(string, "1")
          memory = optional(string, "512Mi")
        }), {})
      }), {})
    }), {})
    process_manager = optional(object({
      replicas     = optional(number, 1)
      min_replicas = optional(number, 1)
      max_replicas = optional(number, 10)
      resources = optional(object({
        requests = optional(object({
          cpu    = optional(string, "100m")
          memory = optional(string, "128Mi")
        }), {})
        limits = optional(object({
          cpu    = optional(string, "1")
          memory = optional(string, "512Mi")
        }), {})
      }), {})
    }), {})
    sagas = optional(map(object({
      replicas     = optional(number, 1)
      min_replicas = optional(number, 1)
      max_replicas = optional(number, 10)
      resources = optional(object({
        requests = optional(object({
          cpu    = optional(string, "100m")
          memory = optional(string, "128Mi")
        }), {})
        limits = optional(object({
          cpu    = optional(string, "500m")
          memory = optional(string, "256Mi")
        }), {})
      }), {})
    })), {})
    projectors = optional(map(object({
      replicas     = optional(number, 1)
      min_replicas = optional(number, 1)
      max_replicas = optional(number, 10)
      resources = optional(object({
        requests = optional(object({
          cpu    = optional(string, "100m")
          memory = optional(string, "128Mi")
        }), {})
        limits = optional(object({
          cpu    = optional(string, "500m")
          memory = optional(string, "256Mi")
        }), {})
      }), {})
    })), {})
  })
  default = {}
}

#------------------------------------------------------------------------------
# Storage
#------------------------------------------------------------------------------
variable "storage" {
  description = "Event storage configuration"
  type = object({
    type = string # mongodb, postgres
    mongodb = optional(object({
      uri      = string
      database = optional(string, "angzarr")
    }), null)
    postgres = optional(object({
      uri = string
    }), null)
  })
}

#------------------------------------------------------------------------------
# Messaging
#------------------------------------------------------------------------------
variable "messaging" {
  description = "Event bus configuration"
  type = object({
    type = string # amqp, kafka
    amqp = optional(object({
      url = string
    }), null)
    kafka = optional(object({
      bootstrap_servers = string
      topic_prefix      = optional(string, "angzarr")
    }), null)
  })
}

#------------------------------------------------------------------------------
# Service Account
#------------------------------------------------------------------------------
variable "create_service_account" {
  description = "Create a service account for this domain"
  type        = bool
  default     = true
}

variable "service_account_name" {
  description = "Kubernetes service account name (if not creating)"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Image Pull
#------------------------------------------------------------------------------
variable "image_pull_secrets" {
  description = "Image pull secret names"
  type        = list(string)
  default     = []
}

#------------------------------------------------------------------------------
# Pod Configuration
#------------------------------------------------------------------------------
variable "node_selector" {
  description = "Node selector for pods"
  type        = map(string)
  default     = {}
}

variable "log_level" {
  description = "Log level (debug, info, warn, error)"
  type        = string
  default     = "info"
}

#------------------------------------------------------------------------------
# Labels
#------------------------------------------------------------------------------
variable "labels" {
  description = "Additional labels to apply to resources"
  type        = map(string)
  default     = {}
}
