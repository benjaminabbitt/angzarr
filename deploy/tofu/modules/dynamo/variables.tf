# DynamoDB Module - Variables

variable "table_prefix" {
  description = "Prefix for DynamoDB table names"
  type        = string
  default     = "angzarr"
}

variable "billing_mode" {
  description = "DynamoDB billing mode: PAY_PER_REQUEST or PROVISIONED"
  type        = string
  default     = "PAY_PER_REQUEST"

  validation {
    condition     = contains(["PAY_PER_REQUEST", "PROVISIONED"], var.billing_mode)
    error_message = "billing_mode must be PAY_PER_REQUEST or PROVISIONED"
  }
}

#------------------------------------------------------------------------------
# Provisioned Capacity (only used if billing_mode = PROVISIONED)
#------------------------------------------------------------------------------

variable "read_capacity" {
  description = "Read capacity units (only for PROVISIONED mode)"
  type        = number
  default     = 5
}

variable "write_capacity" {
  description = "Write capacity units (only for PROVISIONED mode)"
  type        = number
  default     = 5
}

variable "gsi_read_capacity" {
  description = "GSI read capacity units (only for PROVISIONED mode)"
  type        = number
  default     = 5
}

variable "gsi_write_capacity" {
  description = "GSI write capacity units (only for PROVISIONED mode)"
  type        = number
  default     = 5
}

#------------------------------------------------------------------------------
# Security & Durability
#------------------------------------------------------------------------------

variable "point_in_time_recovery" {
  description = "Enable point-in-time recovery"
  type        = bool
  default     = true
}

variable "kms_key_arn" {
  description = "KMS key ARN for encryption (null = AWS managed)"
  type        = string
  default     = null
}

variable "deletion_protection" {
  description = "Enable deletion protection"
  type        = bool
  default     = false
}

#------------------------------------------------------------------------------
# TTL
#------------------------------------------------------------------------------

variable "ttl_attribute" {
  description = "TTL attribute name for events table (null = disabled)"
  type        = string
  default     = null
}

variable "snapshot_ttl_attribute" {
  description = "TTL attribute name for snapshots table (null = disabled)"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------

variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
