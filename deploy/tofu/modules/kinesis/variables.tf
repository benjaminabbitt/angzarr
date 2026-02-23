# Kinesis Module - Variables

#------------------------------------------------------------------------------
# Required
#------------------------------------------------------------------------------
variable "domains" {
  description = "List of domains to create streams for"
  type        = list(string)

  validation {
    condition     = length(var.domains) > 0
    error_message = "At least one domain must be specified."
  }
}

#------------------------------------------------------------------------------
# Stream Configuration
#------------------------------------------------------------------------------
variable "stream_prefix" {
  description = "Prefix for stream names"
  type        = string
  default     = "angzarr"
}

variable "stream_mode" {
  description = "Capacity mode: ON_DEMAND (auto-scaling) or PROVISIONED (fixed shards)"
  type        = string
  default     = "ON_DEMAND"

  validation {
    condition     = contains(["ON_DEMAND", "PROVISIONED"], var.stream_mode)
    error_message = "stream_mode must be ON_DEMAND or PROVISIONED."
  }
}

variable "shard_count" {
  description = "Number of shards (only used with PROVISIONED mode)"
  type        = number
  default     = 1
}

variable "retention_hours" {
  description = "Data retention period in hours (24-8760)"
  type        = number
  default     = 24

  validation {
    condition     = var.retention_hours >= 24 && var.retention_hours <= 8760
    error_message = "retention_hours must be between 24 and 8760 (1 year)."
  }
}

#------------------------------------------------------------------------------
# Encryption
#------------------------------------------------------------------------------
variable "encryption_type" {
  description = "Encryption type: NONE or KMS"
  type        = string
  default     = "KMS"

  validation {
    condition     = contains(["NONE", "KMS"], var.encryption_type)
    error_message = "encryption_type must be NONE or KMS."
  }
}

variable "kms_key_id" {
  description = "KMS key ARN for encryption (null = AWS managed key)"
  type        = string
  default     = null
}

#------------------------------------------------------------------------------
# Dead Letter Queue
#------------------------------------------------------------------------------
variable "enable_dlq" {
  description = "Create DLQ streams for failed events"
  type        = bool
  default     = true
}

variable "dlq_retention_hours" {
  description = "DLQ retention period in hours"
  type        = number
  default     = 168 # 7 days
}

variable "dlq_shard_count" {
  description = "Shard count for DLQ streams (PROVISIONED mode)"
  type        = number
  default     = 1
}

#------------------------------------------------------------------------------
# Enhanced Fan-Out Consumers
# Use for low-latency delivery (<70ms vs ~200ms for standard)
#------------------------------------------------------------------------------
variable "enhanced_fanout_consumers" {
  description = "Map of enhanced fan-out consumer names to their domain"
  type = map(object({
    domain = string
  }))
  default = {}

  # Example:
  # enhanced_fanout_consumers = {
  #   "saga-order-fulfillment" = { domain = "order" }
  #   "projector-order-web"    = { domain = "order" }
  # }
}

#------------------------------------------------------------------------------
# CloudWatch Alarms
#------------------------------------------------------------------------------
variable "enable_alarms" {
  description = "Create CloudWatch alarms for stream health"
  type        = bool
  default     = true
}

variable "iterator_age_threshold_ms" {
  description = "Alarm threshold for consumer lag (milliseconds)"
  type        = number
  default     = 60000 # 1 minute
}

variable "alarm_actions" {
  description = "List of ARNs to notify on alarm (SNS topics, etc.)"
  type        = list(string)
  default     = []
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------
variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
