# Service Mesh module variables

variable "type" {
  description = "Service mesh type: linkerd or istio"
  type        = string
  validation {
    condition     = contains(["linkerd", "istio"], var.type)
    error_message = "Mesh type must be 'linkerd' or 'istio'."
  }
}

variable "namespace" {
  description = "Angzarr namespace to inject mesh"
  type        = string
  default     = "angzarr"
}

variable "inject_namespace" {
  description = "Create and annotate namespace for mesh injection"
  type        = bool
  default     = true
}

variable "linkerd_chart_version" {
  description = "Linkerd Helm chart version"
  type        = string
  default     = "1.16.0"
}

variable "istio_chart_version" {
  description = "Istio Helm chart version"
  type        = string
  default     = "1.22.0"
}

# Linkerd mTLS certificates (required for linkerd)
variable "linkerd_trust_anchor_pem" {
  description = "Linkerd trust anchor certificate PEM"
  type        = string
  default     = ""
  sensitive   = true
}

variable "linkerd_issuer_cert_pem" {
  description = "Linkerd issuer certificate PEM"
  type        = string
  default     = ""
  sensitive   = true
}

variable "linkerd_issuer_key_pem" {
  description = "Linkerd issuer private key PEM"
  type        = string
  default     = ""
  sensitive   = true
}

variable "linkerd_run_as_root" {
  description = "Run linkerd-init as root (required for some CNIs)"
  type        = bool
  default     = false
}

# Resource configuration
variable "proxy_resources" {
  description = "Sidecar proxy resource requests and limits"
  type = object({
    requests = object({
      memory = string
      cpu    = string
    })
    limits = object({
      memory = string
      cpu    = string
    })
  })
  default = {
    requests = {
      memory = "64Mi"
      cpu    = "10m"
    }
    limits = {
      memory = "256Mi"
      cpu    = "1000m"
    }
  }
}

variable "control_plane_resources" {
  description = "Control plane resource requests and limits"
  type = object({
    requests = object({
      memory = string
      cpu    = string
    })
    limits = object({
      memory = string
      cpu    = string
    })
  })
  default = {
    requests = {
      memory = "256Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "1Gi"
      cpu    = "1000m"
    }
  }
}
