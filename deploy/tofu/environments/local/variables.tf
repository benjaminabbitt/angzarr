# Local environment variables
# Passwords are read from K8s secrets - no tfvars needed for credentials

variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/config"
}

variable "kubeconfig_context" {
  description = "Kubeconfig context to use (empty = current context)"
  type        = string
  default     = ""
}

variable "namespace" {
  description = "Kubernetes namespace for angzarr workloads"
  type        = string
  default     = "angzarr"
}

variable "secrets_namespace" {
  description = "Namespace containing the angzarr-secrets secret (created by secrets-init)"
  type        = string
  default     = "angzarr-secrets"
}

# Service mesh (optional for local)
variable "enable_mesh" {
  description = "Enable service mesh (requires mTLS certificates)"
  type        = bool
  default     = false
}

variable "linkerd_trust_anchor_pem" {
  description = "Linkerd trust anchor certificate"
  type        = string
  default     = ""
  sensitive   = true
}

variable "linkerd_issuer_cert_pem" {
  description = "Linkerd issuer certificate"
  type        = string
  default     = ""
  sensitive   = true
}

variable "linkerd_issuer_key_pem" {
  description = "Linkerd issuer private key"
  type        = string
  default     = ""
  sensitive   = true
}
