# AWS Staging Environment - Variables

variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Environment name"
  type        = string
  default     = "staging"
}

variable "image_tag" {
  description = "Container image tag"
  type        = string
  default     = "latest"
}

variable "log_level" {
  description = "Log level for RUST_LOG"
  type        = string
  default     = "info"
}

# Database configuration (for coordinator_env)
variable "database_uri" {
  description = "Database connection URI"
  type        = string
  sensitive   = true
}

variable "messaging_uri" {
  description = "Message broker connection URI"
  type        = string
  sensitive   = true
}
