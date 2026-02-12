# Business Configuration (Portable)
# Defines WHAT the domain does - same structure across all providers

variable "domain" {
  description = "Domain name (e.g., order, inventory, fulfillment)"
  type        = string
}

variable "aggregate" {
  description = "Aggregate configuration - command handler for this domain"
  type = object({
    enabled = bool
    env     = optional(map(string), {})
    upcaster = optional(object({
      enabled = bool
      env     = optional(map(string), {})
    }), { enabled = false })
  })
  default = { enabled = false }
}

variable "process_manager" {
  description = "Process manager configuration - cross-domain orchestrator"
  type = object({
    enabled        = bool
    source_domains = list(string)
    env            = optional(map(string), {})
  })
  default = { enabled = false, source_domains = [] }
}

variable "sagas" {
  description = "Sagas that translate events from this domain to commands in other domains"
  type = map(object({
    target_domain = string
    env           = optional(map(string), {})
  }))
  default = {}
}

variable "projectors" {
  description = "Projectors that build read models from this domain's events"
  type = map(object({
    env = optional(map(string), {})
  }))
  default = {}
}
