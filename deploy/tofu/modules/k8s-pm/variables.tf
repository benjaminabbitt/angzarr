# K8s PM Module - Variables
# Deploys a process manager via Helm

variable "name" {
  description = "Process manager name"
  type        = string
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "image" {
  description = "Process manager logic image"
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
  description = "Additional environment variables"
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

variable "resources" {
  description = "Resource limits and requests"
  type = object({
    requests = optional(object({
      cpu    = optional(string, "100m")
      memory = optional(string, "128Mi")
    }), {})
    limits = optional(object({
      cpu    = optional(string, "500m")
      memory = optional(string, "512Mi")
    }), {})
  })
  default = {}
}
