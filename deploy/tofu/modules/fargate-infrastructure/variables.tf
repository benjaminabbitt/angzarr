# Fargate Infrastructure Module - Variables
# Deploys shared infrastructure services (Stream, Topology)
# AWS Fargate equivalent of the GCP Cloud Run infrastructure module

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

variable "security_group_ids" {
  description = "Security group IDs for Fargate tasks"
  type        = list(string)
  default     = []
}

#------------------------------------------------------------------------------
# Stream Service
#------------------------------------------------------------------------------
variable "stream" {
  description = "Stream service configuration"
  type = object({
    enabled       = bool
    image         = string
    min_instances = optional(number, 0)
    max_instances = optional(number, 10)
    resources = optional(object({
      cpu    = string
      memory = string
    }), { cpu = "1", memory = "512Mi" })
    env = optional(map(string), {})
  })
  default = { enabled = false, image = "" }
}

#------------------------------------------------------------------------------
# Topology Service
#------------------------------------------------------------------------------
variable "topology" {
  description = "Topology service configuration"
  type = object({
    enabled       = bool
    image         = string
    min_instances = optional(number, 0)
    max_instances = optional(number, 5)
    resources = optional(object({
      cpu    = string
      memory = string
    }), { cpu = "0.5", memory = "256Mi" })
    env = optional(map(string), {})
  })
  default = { enabled = false, image = "" }
}

#------------------------------------------------------------------------------
# Shared Configuration
#------------------------------------------------------------------------------
variable "coordinator_env" {
  description = "Shared coordinator environment variables"
  type        = map(string)
  default     = {}
}

variable "coordinator_secrets" {
  description = "Secret environment variables (ARNs to Secrets Manager)"
  type = map(object({
    secret_arn = string
    key        = optional(string)
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
variable "execution_role_arn" {
  description = "Task execution role ARN"
  type        = string
}

variable "task_role_arn" {
  description = "Task role ARN"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Service Discovery
#------------------------------------------------------------------------------
variable "service_discovery_namespace_id" {
  description = "Cloud Map namespace ID"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Load Balancer
#------------------------------------------------------------------------------
variable "lb_arn" {
  description = "Application Load Balancer ARN"
  type        = string
  default     = null
}

variable "lb_listener_arn" {
  description = "ALB listener ARN"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------
variable "tags" {
  description = "Tags to apply to resources"
  type        = map(string)
  default     = {}
}
