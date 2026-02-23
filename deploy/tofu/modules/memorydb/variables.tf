# MemoryDB Module - Variables

variable "cluster_name" {
  description = "MemoryDB cluster name"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "subnet_ids" {
  description = "Subnet IDs for the subnet group"
  type        = list(string)
}

variable "allowed_security_group_ids" {
  description = "Security group IDs allowed to connect"
  type        = list(string)
}

#------------------------------------------------------------------------------
# Cluster Configuration
#------------------------------------------------------------------------------

variable "node_type" {
  description = "MemoryDB node type"
  type        = string
  default     = "db.t4g.small"
}

variable "num_shards" {
  description = "Number of shards"
  type        = number
  default     = 1
}

variable "num_replicas_per_shard" {
  description = "Number of replicas per shard"
  type        = number
  default     = 1
}

#------------------------------------------------------------------------------
# Security
#------------------------------------------------------------------------------

variable "tls_enabled" {
  description = "Enable TLS encryption"
  type        = bool
  default     = true
}

variable "auth_token" {
  description = "Auth token (null = auto-generate)"
  type        = string
  default     = null
  sensitive   = true
}

variable "kms_key_arn" {
  description = "KMS key ARN for encryption (null = AWS managed)"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Maintenance
#------------------------------------------------------------------------------

variable "snapshot_retention_limit" {
  description = "Number of days to retain snapshots"
  type        = number
  default     = 7
}

variable "snapshot_window" {
  description = "Daily time range for snapshots (UTC)"
  type        = string
  default     = "03:00-04:00"
}

variable "maintenance_window" {
  description = "Weekly maintenance window (UTC)"
  type        = string
  default     = "mon:04:00-mon:05:00"
}

variable "auto_minor_version_upgrade" {
  description = "Auto upgrade minor versions"
  type        = bool
  default     = true
}

#------------------------------------------------------------------------------
# Parameters
#------------------------------------------------------------------------------

variable "parameters" {
  description = "Redis parameters"
  type = list(object({
    name  = string
    value = string
  }))
  default = []
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------

variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
