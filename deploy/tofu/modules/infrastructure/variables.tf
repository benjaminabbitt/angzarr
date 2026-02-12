# Infrastructure Module - Variables
# Deploys shared infrastructure services (Stream, Topology)

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
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
  description = "Secret environment variables"
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

variable "vpc_connector" {
  description = "VPC connector for private connectivity"
  type        = string
  default     = null
}

variable "vpc_egress" {
  description = "VPC egress setting"
  type        = string
  default     = "PRIVATE_RANGES_ONLY"
}

variable "service_account" {
  description = "Service account email"
  type        = string
  default     = null
}

variable "allow_unauthenticated" {
  description = "Allow unauthenticated access"
  type        = bool
  default     = false
}

variable "labels" {
  description = "Labels to apply to resources"
  type        = map(string)
  default     = {}
}
