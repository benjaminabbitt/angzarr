# k3s Environment Variables

variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/k3s.yaml"
}

variable "namespace" {
  description = "Kubernetes namespace for angzarr workloads"
  type        = string
  default     = "angzarr"
}

variable "bus_type" {
  description = "Message bus type: rabbit, nats, or kafka"
  type        = string
  default     = "rabbit"

  validation {
    condition     = contains(["rabbit", "nats", "kafka"], var.bus_type)
    error_message = "bus_type must be one of: rabbit, nats, kafka"
  }
}

variable "enable_postgres" {
  description = "Deploy PostgreSQL"
  type        = bool
  default     = true
}

variable "enable_redis" {
  description = "Deploy Redis"
  type        = bool
  default     = true
}

variable "enable_registry" {
  description = "Deploy local container registry"
  type        = bool
  default     = true
}
