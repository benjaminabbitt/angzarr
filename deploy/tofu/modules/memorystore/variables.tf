# Memorystore Module - Variables

variable "name" {
  description = "Memorystore instance name"
  type        = string
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
}

#------------------------------------------------------------------------------
# Instance Configuration
#------------------------------------------------------------------------------

variable "tier" {
  description = "Service tier: BASIC or STANDARD_HA"
  type        = string
  default     = "BASIC"

  validation {
    condition     = contains(["BASIC", "STANDARD_HA"], var.tier)
    error_message = "tier must be BASIC or STANDARD_HA"
  }
}

variable "memory_size_gb" {
  description = "Memory size in GB"
  type        = number
  default     = 1
}

variable "redis_version" {
  description = "Redis version"
  type        = string
  default     = "REDIS_7_0"
}

#------------------------------------------------------------------------------
# Network
#------------------------------------------------------------------------------

variable "authorized_network" {
  description = "VPC network for private access"
  type        = string
}

variable "connect_mode" {
  description = "Connection mode: DIRECT_PEERING or PRIVATE_SERVICE_ACCESS"
  type        = string
  default     = "DIRECT_PEERING"
}

#------------------------------------------------------------------------------
# Security
#------------------------------------------------------------------------------

variable "auth_enabled" {
  description = "Enable AUTH (recommended)"
  type        = bool
  default     = true
}

variable "transit_encryption_mode" {
  description = "TLS mode: SERVER_AUTHENTICATION or DISABLED"
  type        = string
  default     = "SERVER_AUTHENTICATION"
}

#------------------------------------------------------------------------------
# Redis Configuration
#------------------------------------------------------------------------------

variable "redis_configs" {
  description = "Redis configuration parameters"
  type        = map(string)
  default     = {}
}

#------------------------------------------------------------------------------
# Labels
#------------------------------------------------------------------------------

variable "labels" {
  description = "Labels to apply to all resources"
  type        = map(string)
  default     = {}
}
