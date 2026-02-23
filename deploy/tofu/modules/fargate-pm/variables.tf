# Fargate PM Module - Variables

variable "name" {
  description = "Process manager name"
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
  description = "CIDR blocks allowed to access the service"
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
  description = "Resource configuration for task (Fargate CPU/memory units)"
  type = object({
    cpu    = optional(string, "256")
    memory = optional(string, "512")
  })
  default = {}
}

variable "scaling" {
  description = "Scaling configuration"
  type = object({
    desired_count = optional(number, 1)
  })
  default = {}
}
