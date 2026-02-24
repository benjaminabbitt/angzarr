# Infrastructure Module: Registry - Variables

variable "name" {
  description = "Release name for registry"
  type        = string
  default     = "angzarr"
}

variable "image" {
  description = "Registry container image"
  type        = string
  default     = "registry:2"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "node_port" {
  description = "NodePort for external access"
  type        = number
  default     = 30500
}

variable "resources" {
  description = "Resource limits and requests"
  type = object({
    limits = optional(object({
      cpu    = optional(string, "200m")
      memory = optional(string, "256Mi")
    }), {})
    requests = optional(object({
      cpu    = optional(string, "50m")
      memory = optional(string, "64Mi")
    }), {})
  })
  default = {}
}
