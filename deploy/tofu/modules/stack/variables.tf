# Stack Module - Variables
# Orchestrates all domains, process managers, validation, and topology output

variable "name" {
  description = "Stack name (e.g., 'poker', 'ecommerce')"
  type        = string
}

#------------------------------------------------------------------------------
# Compute Configuration
#------------------------------------------------------------------------------

variable "compute" {
  description = <<-EOT
    Compute platform configuration.

    For Kubernetes (any cluster - Kind, GKE, EKS, AKS, OpenShift, Rancher, Tanzu, self-hosted):
      compute_type = "kubernetes"
      namespace    = "angzarr"

    For Cloud Run:
      compute_type    = "cloudrun"
      project_id      = "my-project"
      region          = "us-central1"
      service_account = "sa@project.iam.gserviceaccount.com" (optional)

    For Fargate:
      compute_type       = "fargate"
      cluster_arn        = "arn:aws:ecs:..."
      vpc_id             = "vpc-..."
      subnet_ids         = ["subnet-...", ...]
      region             = "us-east-1"
      execution_role_arn = "arn:aws:iam::..."
      log_group          = "/ecs/angzarr"
  EOT
  type = object({
    compute_type = string
    # K8s-specific
    namespace = optional(string, "angzarr")
    # GCP-specific
    project_id      = optional(string)
    service_account = optional(string)
    # AWS-specific
    cluster_arn        = optional(string)
    vpc_id             = optional(string)
    subnet_ids         = optional(list(string))
    execution_role_arn = optional(string)
    task_role_arn      = optional(string)
    log_group          = optional(string)
    # Shared
    region = optional(string)
  })

  validation {
    condition     = contains(["kubernetes", "cloudrun", "fargate"], var.compute.compute_type)
    error_message = "compute_type must be one of: kubernetes, cloudrun, fargate"
  }
}

#------------------------------------------------------------------------------
# Bus Configuration
#------------------------------------------------------------------------------

variable "bus" {
  description = <<-EOT
    Event bus configuration.

    For Kubernetes (NATS, Kafka, RabbitMQ):
      type           = "nats" | "kafka" | "rabbit"
      connection_uri = "nats://nats.angzarr.svc:4222"
      provides       = { capabilities = ["event_bus"], rust_features = ["nats"] }

    For AWS (Kinesis, SNS/SQS):
      type           = "kinesis" | "sns-sqs"
      connection_uri = "kinesis://..."
      provides       = { ... }

    For GCP (Pub/Sub):
      type           = "pubsub"
      connection_uri = "pubsub://project/..."
      provides       = { ... }
  EOT
  type = object({
    type           = string
    connection_uri = string
    provides = object({
      capabilities  = set(string)
      rust_features = set(string)
    })
  })

  validation {
    condition     = contains(var.bus.provides.capabilities, "event_bus")
    error_message = "Bus must provide 'event_bus' capability"
  }
}

#------------------------------------------------------------------------------
# Storage Configuration
#------------------------------------------------------------------------------

variable "default_storage" {
  description = <<-EOT
    Default storage backends for all domains. Individual domains can override.

    event_store and position_store are required.
    snapshot_store is optional (set to null to disable snapshots).
  EOT
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

  validation {
    condition     = contains(var.default_storage.event_store.provides.capabilities, "event_store")
    error_message = "event_store must provide 'event_store' capability"
  }

  validation {
    condition     = contains(var.default_storage.position_store.provides.capabilities, "position_store")
    error_message = "position_store must provide 'position_store' capability"
  }
}

#------------------------------------------------------------------------------
# Domains
#------------------------------------------------------------------------------

variable "domains" {
  description = <<-EOT
    Domain configurations. Each domain contains:
    - aggregate: Command handler configuration
    - sagas: Map of saga name to saga config (translates events to other domains)
    - projectors: Map of projector name to projector config (optional)
    - storage: Optional storage override (null = use default_storage)
    - entry_point: Mark as an entry-point domain to exempt from orphan validation (default: false)
  EOT
  type = map(object({
    aggregate = object({
      image = string
      env   = optional(map(string), {})
    })
    sagas = optional(map(object({
      target_domain = string
      image         = string
      env           = optional(map(string), {})
    })), {})
    projectors = optional(map(object({
      image = string
      env   = optional(map(string), {})
    })), {})
    storage = optional(object({
      event_store = optional(object({
        connection_uri = string
        provides = object({
          capabilities  = set(string)
          rust_features = set(string)
        })
      }))
      position_store = optional(object({
        connection_uri = string
        provides = object({
          capabilities  = set(string)
          rust_features = set(string)
        })
      }))
      snapshot_store = optional(object({
        connection_uri = string
        provides = object({
          capabilities  = set(string)
          rust_features = set(string)
        })
      }))
    }))
    entry_point = optional(bool, false)
  }))

  validation {
    condition     = length(var.domains) > 0
    error_message = "At least one domain must be defined"
  }
}

#------------------------------------------------------------------------------
# Process Managers
#------------------------------------------------------------------------------

variable "process_managers" {
  description = <<-EOT
    Process manager configurations. PMs orchestrate across multiple domains.
    - subscriptions: List of domains to subscribe to
    - targets: List of domains to emit commands to
    - storage: Optional storage override (null = use default_storage)
  EOT
  type = map(object({
    image         = string
    subscriptions = list(string)
    targets       = list(string)
    env           = optional(map(string), {})
    storage = optional(object({
      event_store = optional(object({
        connection_uri = string
        provides = object({
          capabilities  = set(string)
          rust_features = set(string)
        })
      }))
      position_store = optional(object({
        connection_uri = string
        provides = object({
          capabilities  = set(string)
          rust_features = set(string)
        })
      }))
      snapshot_store = optional(object({
        connection_uri = string
        provides = object({
          capabilities  = set(string)
          rust_features = set(string)
        })
      }))
    }))
  }))
  default = {}
}

#------------------------------------------------------------------------------
# Coordinator Images
#------------------------------------------------------------------------------

variable "coordinator_images" {
  description = "Coordinator container images for each component type"
  type = object({
    aggregate    = string
    saga         = string
    projector    = string
    pm           = string
    grpc_gateway = optional(string)
  })
  default = {
    aggregate    = "ghcr.io/angzarr-io/coordinator-aggregate:latest"
    saga         = "ghcr.io/angzarr-io/coordinator-saga:latest"
    projector    = "ghcr.io/angzarr-io/coordinator-projector:latest"
    pm           = "ghcr.io/angzarr-io/coordinator-pm:latest"
    grpc_gateway = "ghcr.io/angzarr-io/grpc-gateway:latest"
  }
}

#------------------------------------------------------------------------------
# Labels
#------------------------------------------------------------------------------

variable "labels" {
  description = "Labels to apply to all resources"
  type        = map(string)
  default     = {}
}
