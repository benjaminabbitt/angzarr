# Cloud Run PM Module - Variables

variable "name" {
  description = "Process manager name"
  type        = string
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
}

variable "image" {
  description = "Business logic image"
  type        = string
}

variable "subscriptions" {
  description = "Domains to subscribe to"
  type        = list(string)
}

variable "targets" {
  description = "Domains to emit commands to"
  type        = list(string)
}

variable "env" {
  description = "Environment variables for business logic"
  type        = map(string)
  default     = {}
}

variable "storage" {
  description = "Resolved storage configuration"
  type = object({
    event_store = object({
      connection_uri = string
      provides = object({
        capabilities  = set(string)
        rust_features = set(string)
      })
    })
    position_store = object({
      connection_uri = string
      provides = object({
        capabilities  = set(string)
        rust_features = set(string)
      })
    })
    snapshot_store = optional(object({
      connection_uri = string
      provides = object({
        capabilities  = set(string)
        rust_features = set(string)
      })
    }))
  })
}

variable "bus" {
  description = "Event bus configuration"
  type = object({
    type           = string
    connection_uri = string
    provides = object({
      capabilities  = set(string)
      rust_features = set(string)
    })
  })
}

variable "coordinator_images" {
  description = "Coordinator container images"
  type = object({
    aggregate    = string
    saga         = string
    projector    = string
    pm           = string
    grpc_gateway = optional(string)
  })
}

variable "labels" {
  description = "Labels to apply to resources"
  type        = map(string)
  default     = {}
}

variable "service_account" {
  description = "Service account email for Cloud Run service"
  type        = string
  default     = null
}

variable "resources" {
  description = "Resource limits for containers"
  type = object({
    coordinator = optional(object({
      cpu    = optional(string, "1")
      memory = optional(string, "512Mi")
    }), {})
    logic = optional(object({
      cpu    = optional(string, "1")
      memory = optional(string, "512Mi")
    }), {})
  })
  default = {}
}

variable "scaling" {
  description = "Scaling configuration"
  type = object({
    min_instances = optional(number, 0)
    max_instances = optional(number, 10)
  })
  default = {}
}
