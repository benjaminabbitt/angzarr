# Example K8s Environment - Variables

variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/config"
}

variable "kubeconfig_context" {
  description = "Kubeconfig context to use (e.g., 'kind-angzarr', 'gke_project_region_cluster')"
  type        = string
  default     = ""
}

variable "namespace" {
  description = "Kubernetes namespace for angzarr"
  type        = string
  default     = "angzarr"
}

variable "stack_name" {
  description = "Name for the angzarr stack"
  type        = string
  default     = "angzarr"
}

variable "use_redis_snapshots" {
  description = "Use Redis for snapshot store (faster reads)"
  type        = bool
  default     = false
}

variable "coordinator_images" {
  description = "Container images for coordinators"
  type = object({
    aggregate    = string
    saga         = string
    projector    = string
    pm           = string
    grpc_gateway = optional(string)
  })
  default = {
    aggregate    = "ghcr.io/benjaminabbitt/angzarr-aggregate:latest"
    saga         = "ghcr.io/benjaminabbitt/angzarr-saga:latest"
    projector    = "ghcr.io/benjaminabbitt/angzarr-projector:latest"
    pm           = "ghcr.io/benjaminabbitt/angzarr-pm:latest"
    grpc_gateway = null
  }
}

variable "domains" {
  description = "Domain configurations"
  type = map(object({
    aggregate = object({
      image = string
      env   = optional(map(string), {})
    })
    sagas = optional(map(object({
      target_domain = string
      image         = string
      env           = optional(map(string), {})
    })), {})
    projectors = optional(map(object({
      image = string
      env   = optional(map(string), {})
    })), {})
  }))
  default = {}
}

variable "process_managers" {
  description = "Process manager configurations"
  type = map(object({
    image         = string
    subscriptions = list(string)
    targets       = list(string)
    env           = optional(map(string), {})
  }))
  default = {}
}
