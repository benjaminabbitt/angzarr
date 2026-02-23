# GKE Base Module - Variables

variable "cluster_name" {
  description = "GKE cluster name"
  type        = string
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region (regional cluster for HA)"
  type        = string
}

#------------------------------------------------------------------------------
# Network Configuration
#------------------------------------------------------------------------------

variable "create_network" {
  description = "Create a new VPC network"
  type        = bool
  default     = true
}

variable "network" {
  description = "Existing VPC network name (required if create_network = false)"
  type        = string
  default     = null
}

variable "subnetwork" {
  description = "Existing subnetwork name (required if create_network = false)"
  type        = string
  default     = null
}

variable "subnet_cidr" {
  description = "Subnet CIDR (only for create_network = true)"
  type        = string
  default     = "10.0.0.0/20"
}

variable "pods_cidr" {
  description = "Pods secondary range CIDR (only for create_network = true)"
  type        = string
  default     = "10.1.0.0/16"
}

variable "services_cidr" {
  description = "Services secondary range CIDR (only for create_network = true)"
  type        = string
  default     = "10.2.0.0/20"
}

variable "pods_range_name" {
  description = "Pods secondary range name (only for create_network = false)"
  type        = string
  default     = null
}

variable "services_range_name" {
  description = "Services secondary range name (only for create_network = false)"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Cluster Configuration
#------------------------------------------------------------------------------

variable "release_channel" {
  description = "Release channel: RAPID, REGULAR, or STABLE"
  type        = string
  default     = "REGULAR"
}

variable "enable_private_nodes" {
  description = "Enable private nodes (no public IPs)"
  type        = bool
  default     = true
}

variable "enable_private_endpoint" {
  description = "Enable private endpoint (no public master)"
  type        = bool
  default     = false
}

variable "master_ipv4_cidr_block" {
  description = "CIDR block for master (required if private nodes enabled)"
  type        = string
  default     = "172.16.0.0/28"
}

variable "enable_network_policy" {
  description = "Enable network policy (Calico)"
  type        = bool
  default     = false
}

variable "deletion_protection" {
  description = "Enable deletion protection"
  type        = bool
  default     = false
}

#------------------------------------------------------------------------------
# Node Pools
#------------------------------------------------------------------------------

variable "node_pools" {
  description = <<-EOT
    Map of node pool configurations.

    Example:
      {
        default = {
          machine_type = "e2-small"
          disk_size_gb = 50
          node_count   = 2
          # OR autoscaling:
          autoscaling  = { min_nodes = 1, max_nodes = 5 }
          labels       = { workload = "default" }
          taints       = []
        }
      }
  EOT
  type = map(object({
    machine_type = optional(string)
    disk_size_gb = optional(number)
    disk_type    = optional(string)
    node_count   = optional(number)
    autoscaling = optional(object({
      min_nodes = number
      max_nodes = number
    }))
    labels = optional(map(string))
    taints = optional(list(object({
      key    = string
      value  = optional(string)
      effect = string
    })))
  }))
  default = {
    default = {
      machine_type = "e2-small"
      node_count   = 1
    }
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
