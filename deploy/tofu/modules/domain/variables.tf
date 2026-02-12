# Domain Module - Variables
# Deploys all components for a single domain (aggregate/PM + sagas + projectors)
#
# Variables are separated into:
# - Business config: Domain logic, env vars, component relationships (portable)
# - Operational config: Images, scaling, resources, networking (provider-specific)

variable "domain" {
  description = "Domain name (e.g., order, inventory, fulfillment)"
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

#==============================================================================
# BUSINESS CONFIG (Portable)
# These configs are independent of deployment platform
#==============================================================================

variable "aggregate" {
  description = "Aggregate configuration (mutually exclusive with process_manager)"
  type = object({
    enabled = bool
    env     = optional(map(string), {})
    upcaster = optional(object({
      enabled = bool
      env     = optional(map(string), {})
    }), { enabled = false })
  })
  default = { enabled = false }
}

variable "process_manager" {
  description = "Process manager configuration (mutually exclusive with aggregate)"
  type = object({
    enabled        = bool
    source_domains = list(string)
    env            = optional(map(string), {})
  })
  default = { enabled = false, source_domains = [] }
}

variable "sagas" {
  description = "Sagas that source events from this domain"
  type = map(object({
    target_domain = string
    env           = optional(map(string), {})
  }))
  default = {}
}

variable "projectors" {
  description = "Projectors that source events from this domain"
  type = map(object({
    env = optional(map(string), {})
  }))
  default = {}
}

#==============================================================================
# OPERATIONAL CONFIG (Provider-specific)
# These configs are specific to GCP Cloud Run deployment
#==============================================================================

#------------------------------------------------------------------------------
# Container Images
#------------------------------------------------------------------------------
variable "images" {
  description = "Container images for all components"
  type = object({
    grpc_gateway          = string                       # REST bridge
    coordinator_aggregate = string                       # Aggregate coordinator
    coordinator_saga      = string                       # Saga coordinator
    coordinator_projector = string                       # Projector coordinator
    coordinator_pm        = string                       # Process manager coordinator
    logic                 = string                       # Domain business logic
    upcaster              = optional(string)             # Upcaster (if enabled)
    saga_logic            = optional(map(string), {})    # saga name → image
    projector_logic       = optional(map(string), {})    # projector name → image
  })
}

#------------------------------------------------------------------------------
# Scaling & Resources
#------------------------------------------------------------------------------
variable "scaling" {
  description = "Scaling and resource configuration per component type"
  type = object({
    aggregate = optional(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "1", memory = "512Mi" })
    }), {})
    process_manager = optional(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "1", memory = "512Mi" })
    }), {})
    sagas = optional(map(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "1", memory = "256Mi" })
    })), {})
    projectors = optional(map(object({
      min_instances = optional(number, 0)
      max_instances = optional(number, 10)
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "1", memory = "256Mi" })
    })), {})
    upcaster = optional(object({
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "0.5", memory = "128Mi" })
    }), {})
    grpc_gateway = optional(object({
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "0.5", memory = "128Mi" })
    }), {})
    coordinator = optional(object({
      resources = optional(object({
        cpu    = string
        memory = string
      }), { cpu = "1", memory = "512Mi" })
    }), {})
  })
  default = {}
}

#------------------------------------------------------------------------------
# Networking
#------------------------------------------------------------------------------
variable "networking" {
  description = "Networking configuration"
  type = object({
    vpc_connector = optional(string)
    vpc_egress    = optional(string, "PRIVATE_RANGES_ONLY")
  })
  default = {}
}

#------------------------------------------------------------------------------
# Execution Settings
#------------------------------------------------------------------------------
variable "execution" {
  description = "Cloud Run execution settings"
  type = object({
    environment     = optional(string, "EXECUTION_ENVIRONMENT_GEN2")
    timeout_seconds = optional(number, 300)
    cpu_idle        = optional(bool, false)
  })
  default = {}
}

#------------------------------------------------------------------------------
# Coordinator Configuration
#------------------------------------------------------------------------------
variable "discovery_env" {
  description = "Service discovery environment variables (from registry module)"
  type        = map(string)
  default     = {}
}

variable "coordinator_env" {
  description = "Shared coordinator environment variables (storage, messaging)"
  type        = map(string)
  default     = {}
}

variable "coordinator_secrets" {
  description = "Secret environment variables for coordinator"
  type = map(object({
    secret  = string
    version = string
  }))
  default = {}
}

variable "log_level" {
  description = "Log level for RUST_LOG"
  type        = string
  default     = "info"
}

#------------------------------------------------------------------------------
# IAM
#------------------------------------------------------------------------------
variable "iam" {
  description = "IAM configuration"
  type = object({
    create_service_account = optional(bool, true)
    service_account        = optional(string)
    allow_unauthenticated  = optional(bool, false)
  })
  default = {}
}

#------------------------------------------------------------------------------
# Labels
#------------------------------------------------------------------------------
variable "labels" {
  description = "Additional labels to apply to resources"
  type        = map(string)
  default     = {}
}
