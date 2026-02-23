# EKS Base Module - Variables

variable "cluster_name" {
  description = "EKS cluster name"
  type        = string
}

variable "cluster_version" {
  description = "Kubernetes version"
  type        = string
  default     = "1.29"
}

#------------------------------------------------------------------------------
# VPC Configuration
#------------------------------------------------------------------------------

variable "create_vpc" {
  description = "Create a new VPC (false = use existing)"
  type        = bool
  default     = true
}

variable "vpc_id" {
  description = "Existing VPC ID (required if create_vpc = false)"
  type        = string
  default     = null
}

variable "subnet_ids" {
  description = "Existing subnet IDs (required if create_vpc = false)"
  type        = list(string)
  default     = null
}

variable "vpc_cidr" {
  description = "VPC CIDR block (only for create_vpc = true)"
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "Availability zones (only for create_vpc = true)"
  type        = list(string)
  default     = ["us-east-1a", "us-east-1b", "us-east-1c"]
}

variable "single_nat_gateway" {
  description = "Use single NAT gateway (cost savings, less HA)"
  type        = bool
  default     = true
}

#------------------------------------------------------------------------------
# Cluster Configuration
#------------------------------------------------------------------------------

variable "endpoint_public_access" {
  description = "Enable public access to cluster endpoint"
  type        = bool
  default     = true
}

variable "enabled_cluster_log_types" {
  description = "Cluster log types to enable"
  type        = list(string)
  default     = ["api", "audit"]
}

#------------------------------------------------------------------------------
# Node Groups
#------------------------------------------------------------------------------

variable "node_groups" {
  description = <<-EOT
    Map of node group configurations.

    Example:
      {
        default = {
          instance_types = ["t3.medium"]
          min_size       = 2
          max_size       = 10
          desired_size   = 2
          capacity_type  = "ON_DEMAND"
          labels         = { workload = "default" }
          taints         = []
        }
      }
  EOT
  type = map(object({
    instance_types = list(string)
    min_size       = number
    max_size       = number
    desired_size   = optional(number)
    capacity_type  = optional(string)
    labels         = optional(map(string))
    taints = optional(list(object({
      key    = string
      value  = optional(string)
      effect = string
    })))
  }))
  default = {
    default = {
      instance_types = ["t3.small"]
      min_size       = 1
      max_size       = 3
    }
  }
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------

variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
