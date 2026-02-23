# Cloud Run Domain Module - Variables

variable "domain" {
  description = "Domain name"
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

variable "aggregate" {
  description = "Aggregate configuration"
  type = object({
    image = string
    env   = optional(map(string), {})
  })
}

variable "sagas" {
  description = "Saga configurations"
  type = map(object({
    target_domain = string
    image         = string
    env           = optional(map(string), {})
  }))
  default = {}
}

variable "projectors" {
  description = "Projector configurations"
  type = map(object({
    image = string
    env   = optional(map(string), {})
  }))
  default = {}
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
  description = "Service account email for Cloud Run services"
  type        = string
  default     = null
}

variable "resources" {
  description = "Resource limits for containers"
  type = object({
    aggregate = optional(object({
      coordinator = optional(object({
        cpu    = optional(string, "1")
        memory = optional(string, "512Mi")
      }), {})
      logic = optional(object({
        cpu    = optional(string, "1")
        memory = optional(string, "512Mi")
      }), {})
    }), {})
    saga = optional(object({
      coordinator = optional(object({
        cpu    = optional(string, "1")
        memory = optional(string, "256Mi")
      }), {})
      logic = optional(object({
        cpu    = optional(string, "1")
        memory = optional(string, "256Mi")
      }), {})
    }), {})
    projector = optional(object({
      coordinator = optional(object({
        cpu    = optional(string, "1")
        memory = optional(string, "256Mi")
      }), {})
      logic = optional(object({
        cpu    = optional(string, "1")
        memory = optional(string, "256Mi")
      }), {})
    }), {})
  })
  default = {}
}

variable "scaling" {
  description = "Scaling configuration"
  type = object({
    aggregate = optional(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
    }), {})
    saga = optional(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
    }), {})
    projector = optional(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
    }), {})
  })
  default = {}
}
