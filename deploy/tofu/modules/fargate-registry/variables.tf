# Fargate Registry Module - Variables
# Aggregates service discovery entries from all domains
# AWS equivalent of the GCP registry module

variable "services" {
  description = "Map of service discovery entries from all domain modules"
  type        = map(string)
  default     = {}
}

variable "stream_dns" {
  description = "DNS name of the stream service (optional)"
  type        = string
  default     = null
}

variable "topology_dns" {
  description = "DNS name of the topology service (optional)"
  type        = string
  default     = null
}

variable "namespace_name" {
  description = "Cloud Map namespace name for constructing full DNS names"
  type        = string
  default     = null
}

variable "additional_env" {
  description = "Additional environment variables to include in discovery"
  type        = map(string)
  default     = {}
}
