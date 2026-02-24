# Fargate Domain Module - Variables

variable "domain" {
  description = "Domain name"
  type        = string
}

variable "cluster_arn" {
  description = "ECS cluster ARN"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "subnet_ids" {
  description = "Subnet IDs for Fargate tasks"
  type        = list(string)
}

variable "region" {
  description = "AWS region"
  type        = string
  default     = null
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
  description = "Tags to apply to resources"
  type        = map(string)
  default     = {}
}

variable "execution_role_arn" {
  description = "ECS task execution role ARN"
  type        = string
}

variable "task_role_arn" {
  description = "ECS task role ARN"
  type        = string
  default     = null
}

variable "log_group" {
  description = "CloudWatch log group name"
  type        = string
}

variable "allowed_cidr_blocks" {
  description = "CIDR blocks allowed to access the services"
  type        = list(string)
  default     = ["10.0.0.0/8"]
}

variable "assign_public_ip" {
  description = "Assign public IP to tasks"
  type        = bool
  default     = false
}

variable "service_discovery_namespace_id" {
  description = "Cloud Map namespace ID for service discovery"
  type        = string
  default     = null
}

variable "resources" {
  description = "Resource configuration for tasks (Fargate CPU/memory units)"
  type = object({
    aggregate = optional(object({
      cpu    = optional(string, "256")
      memory = optional(string, "512")
    }), {})
    saga = optional(object({
      cpu    = optional(string, "256")
      memory = optional(string, "512")
    }), {})
    projector = optional(object({
      cpu    = optional(string, "256")
      memory = optional(string, "512")
    }), {})
  })
  default = {}
}

variable "scaling" {
  description = "Scaling configuration"
  type = object({
    aggregate = optional(object({
      desired_count = optional(number, 1)
    }), {})
    saga = optional(object({
      desired_count = optional(number, 1)
    }), {})
    projector = optional(object({
      desired_count = optional(number, 1)
    }), {})
  })
  default = {}
}

variable "grpc_gateway" {
  description = "gRPC Gateway (REST proxy) configuration"
  type = object({
    enabled = optional(bool, false)
    port    = optional(number, 8080)
  })
  default = {}
}
