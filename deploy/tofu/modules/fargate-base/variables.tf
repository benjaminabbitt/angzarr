# Fargate Base Module - Variables
# Shared infrastructure: VPC, ECS Cluster, Cloud Map, ALB, IAM

variable "name" {
  description = "Name prefix for all resources"
  type        = string
  default     = "angzarr"
}

variable "environment" {
  description = "Environment name (e.g., staging, prod)"
  type        = string
}

#------------------------------------------------------------------------------
# VPC Configuration
#------------------------------------------------------------------------------
variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "List of availability zones"
  type        = list(string)
  default     = ["us-east-1a", "us-east-1b"]
}

variable "create_vpc" {
  description = "Create a new VPC (false to use existing)"
  type        = bool
  default     = true
}

variable "existing_vpc_id" {
  description = "Existing VPC ID (if create_vpc is false)"
  type        = string
  default     = null
}

variable "existing_private_subnet_ids" {
  description = "Existing private subnet IDs (if create_vpc is false)"
  type        = list(string)
  default     = []
}

variable "existing_public_subnet_ids" {
  description = "Existing public subnet IDs (if create_vpc is false)"
  type        = list(string)
  default     = []
}

#------------------------------------------------------------------------------
# ALB Configuration
#------------------------------------------------------------------------------
variable "create_alb" {
  description = "Create an Application Load Balancer"
  type        = bool
  default     = true
}

variable "alb_internal" {
  description = "Make ALB internal (not internet-facing)"
  type        = bool
  default     = false
}

variable "alb_certificate_arn" {
  description = "ACM certificate ARN for HTTPS"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Service Discovery
#------------------------------------------------------------------------------
variable "create_service_discovery" {
  description = "Create Cloud Map namespace for service discovery"
  type        = bool
  default     = true
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------
variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
