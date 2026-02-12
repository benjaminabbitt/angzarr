# Operational Configuration (AWS Fargate Native)
# Defines HOW the domain runs - provider-specific

#------------------------------------------------------------------------------
# Images
#------------------------------------------------------------------------------
variable "images" {
  description = "Container images for all components"
  type = object({
    grpc_gateway          = string
    coordinator_aggregate = string
    coordinator_saga      = string
    coordinator_projector = string
    coordinator_pm        = string
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
      min_instances = optional(number, 1)
      max_instances = optional(number, 10)
      cpu           = optional(number, 1024)
      memory        = optional(number, 512)
    }), {})
    process_manager = optional(object({
      min_instances = optional(number, 1)
      max_instances = optional(number, 10)
      cpu           = optional(number, 1024)
      memory        = optional(number, 512)
    }), {})
    sagas = optional(map(object({
      min_instances = optional(number, 1)
      max_instances = optional(number, 10)
      cpu           = optional(number, 1024)
      memory        = optional(number, 256)
    })), {})
    projectors = optional(map(object({
      min_instances = optional(number, 1)
      max_instances = optional(number, 10)
      cpu           = optional(number, 1024)
      memory        = optional(number, 256)
    })), {})
  })
  default = {}
}

#------------------------------------------------------------------------------
# Networking (AWS VPC)
#------------------------------------------------------------------------------
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

variable "assign_public_ip" {
  description = "Assign public IP to tasks"
  type        = bool
  default     = false
}

#------------------------------------------------------------------------------
# Service Discovery (AWS Cloud Map)
#------------------------------------------------------------------------------
variable "service_discovery_namespace_id" {
  description = "Cloud Map namespace ID for service discovery"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Load Balancer (AWS ALB)
#------------------------------------------------------------------------------
variable "lb_arn" {
  description = "Application Load Balancer ARN"
  type        = string
  default     = null
}

variable "lb_listener_arn" {
  description = "ALB listener ARN for adding target groups"
  type        = string
  default     = null
}

variable "lb_listener_priority_base" {
  description = "Base priority for listener rules"
  type        = number
  default     = 100
}

#------------------------------------------------------------------------------
# IAM (AWS Native)
#------------------------------------------------------------------------------
variable "execution_role_arn" {
  description = "Task execution role ARN (for pulling images, logging)"
  type        = string
}

variable "create_task_role" {
  description = "Create a task role for this domain"
  type        = bool
  default     = true
}

variable "task_role_arn" {
  description = "Existing task role ARN (if not creating)"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Secrets (AWS Secrets Manager)
#------------------------------------------------------------------------------
variable "coordinator_secrets" {
  description = "Secret environment variables (Secrets Manager ARNs)"
  type = map(object({
    secret_arn = string
    key        = optional(string)
  }))
  default = {}
}

#------------------------------------------------------------------------------
# Coordinator Environment
#------------------------------------------------------------------------------
variable "coordinator_env" {
  description = "Shared coordinator environment variables (storage, messaging)"
  type        = map(string)
  default     = {}
}

variable "discovery_env" {
  description = "Service discovery environment variables"
  type        = map(string)
  default     = {}
}

variable "log_level" {
  description = "Log level for RUST_LOG"
  type        = string
  default     = "info"
}

#------------------------------------------------------------------------------
# Execution
#------------------------------------------------------------------------------
variable "health_check_grace_period" {
  description = "Health check grace period in seconds"
  type        = number
  default     = 300
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------
variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
