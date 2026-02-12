# Registry Module - Variables
# Aggregates service URLs from all domains for service discovery

variable "services" {
  description = "Map of service discovery entries from all domain modules"
  type        = map(string)
  default     = {}
}

variable "stream_url" {
  description = "URL of the stream service (optional)"
  type        = string
  default     = null
}

variable "topology_url" {
  description = "URL of the topology service (optional)"
  type        = string
  default     = null
}

variable "additional_env" {
  description = "Additional environment variables to include in discovery"
  type        = map(string)
  default     = {}
}
