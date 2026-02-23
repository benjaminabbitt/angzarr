# GitHub environment variables

variable "github_owner" {
  description = "GitHub organization or user"
  type        = string
  default     = "angzarr-io"
}

variable "repository_name" {
  description = "Repository name"
  type        = string
  default     = "angzarr"
}

# =============================================================================
# Repository-level secrets
# =============================================================================

variable "actions_secrets" {
  description = "Repository-level Actions secrets"
  type        = map(string)
  default     = {}
  sensitive   = true
}

# =============================================================================
# Staging environment
# =============================================================================

variable "staging_secrets" {
  description = "Staging environment secrets"
  type        = map(string)
  default     = {}
  sensitive   = true
}

variable "staging_variables" {
  description = "Staging environment variables"
  type        = map(string)
  default     = {}
}

# =============================================================================
# Production environment
# =============================================================================

variable "production_secrets" {
  description = "Production environment secrets"
  type        = map(string)
  default     = {}
  sensitive   = true
}

variable "production_variables" {
  description = "Production environment variables"
  type        = map(string)
  default     = {}
}

# =============================================================================
# Webhooks
# =============================================================================

variable "webhooks" {
  description = "Repository webhooks"
  type = map(object({
    url          = string
    content_type = optional(string, "json")
    insecure_ssl = optional(bool, false)
    secret       = optional(string, null)
    active       = optional(bool, true)
    events       = list(string)
  }))
  default   = {}
  sensitive = true
}
