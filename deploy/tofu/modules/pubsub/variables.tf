# Pub/Sub Module - Variables

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "events_topic_name" {
  description = "Name of the main events topic"
  type        = string
  default     = "angzarr-events"
}

variable "domains" {
  description = "List of domains for per-domain topics"
  type        = list(string)
  default     = []
}

variable "create_domain_topics" {
  description = "Create separate topics per domain"
  type        = bool
  default     = false
}

# Message configuration
variable "message_retention_duration" {
  description = "How long to retain messages (max 31 days)"
  type        = string
  default     = "86400s" # 1 day
}

variable "schema_id" {
  description = "Schema ID for message validation (optional)"
  type        = string
  default     = null
}

# Push subscribers (sagas, projectors)
variable "push_subscribers" {
  description = "Map of push subscribers with their endpoints"
  type = map(object({
    endpoint        = string           # Cloud Run URL
    service_account = optional(string) # For OIDC auth
    type            = optional(string) # saga, projector, process_manager
    domain_filter   = optional(string) # Filter by domain attribute
  }))
  default = {}
}

# Pull subscription
variable "create_pull_subscription" {
  description = "Create a pull subscription for debugging/replay"
  type        = bool
  default     = false
}

# Dead letter
variable "enable_dead_letter" {
  description = "Enable dead letter topic for failed messages"
  type        = bool
  default     = true
}

variable "max_delivery_attempts" {
  description = "Max delivery attempts before dead lettering"
  type        = number
  default     = 5
}

# Retry configuration
variable "ack_deadline_seconds" {
  description = "Ack deadline in seconds"
  type        = number
  default     = 60
}

variable "retry_minimum_backoff" {
  description = "Minimum backoff duration"
  type        = string
  default     = "10s"
}

variable "retry_maximum_backoff" {
  description = "Maximum backoff duration"
  type        = string
  default     = "600s"
}

# IAM
variable "publishers" {
  description = "IAM members allowed to publish (service accounts for aggregates)"
  type        = list(string)
  default     = []
}

variable "push_subscriber_accounts" {
  description = "Service accounts that will receive push messages"
  type        = list(string)
  default     = []
}

variable "grant_cloudrun_invoker" {
  description = "Grant Cloud Run invoker role to push subscriber accounts"
  type        = bool
  default     = false
}

# Labels
variable "labels" {
  description = "Labels to apply to resources"
  type        = map(string)
  default     = {}
}
