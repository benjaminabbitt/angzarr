# SNS/SQS Module - Variables

variable "name_prefix" {
  description = "Prefix for resource names"
  type        = string
  default     = "angzarr"
}

variable "domains" {
  description = "List of domain names to create SNS topics for"
  type        = list(string)
}

variable "subscribers" {
  description = <<-EOT
    Map of subscriber name to list of domains they subscribe to.

    Example:
      {
        "saga-order-fulfillment" = ["order"]
        "pm-hand-flow"           = ["player", "table", "hand"]
      }
  EOT
  type        = map(list(string))
}

variable "fifo" {
  description = <<-EOT
    Use FIFO topics and queues.

    FIFO provides ordering and exactly-once delivery but has lower throughput.
    Standard queues work fine - angzarr handles idempotency at the application layer.
  EOT
  type        = bool
  default     = false
}

variable "message_retention_seconds" {
  description = "How long messages are retained in queues (seconds)"
  type        = number
  default     = 345600 # 4 days
}

variable "visibility_timeout_seconds" {
  description = "How long a message is hidden after being received (seconds)"
  type        = number
  default     = 30
}

variable "max_receive_count" {
  description = "Number of receive attempts before sending to DLQ"
  type        = number
  default     = 5
}

variable "enable_dlq" {
  description = "Create dead letter queues"
  type        = bool
  default     = true
}

variable "enable_alarms" {
  description = "Create CloudWatch alarms for DLQs"
  type        = bool
  default     = true
}

variable "kms_key_id" {
  description = "KMS key ID for encryption (null = AWS managed)"
  type        = string
  default     = null
}

variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
