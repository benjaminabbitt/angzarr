# Staging environment variables
# Passwords can come from K8s secrets (default) or tfvars (for cloud-managed)

variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/config"
}

variable "kubeconfig_context" {
  description = "Kubeconfig context to use"
  type        = string
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
  default     = "angzarr-staging"
}

# Secrets source
variable "use_k8s_secrets" {
  description = "Read passwords from K8s secrets (created by secrets-init). Set to false for cloud-managed services."
  type        = bool
  default     = true
}

variable "secrets_namespace" {
  description = "Namespace containing the angzarr-secrets secret"
  type        = string
  default     = "angzarr-secrets"
}

# Database
variable "database_type" {
  description = "Database type: postgresql or mongodb"
  type        = string
  default     = "postgresql"
}

variable "database_managed" {
  description = "Use cloud-managed database"
  type        = bool
  default     = false
}

# These are only needed if use_k8s_secrets = false
variable "db_admin_password" {
  description = "Database admin password (only used if use_k8s_secrets = false)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "db_password" {
  description = "Database application password (only used if use_k8s_secrets = false)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "db_external_host" {
  description = "External database host (when managed=true)"
  type        = string
  default     = ""
}

variable "db_external_port" {
  description = "External database port (when managed=true)"
  type        = number
  default     = 5432
}

variable "db_external_uri" {
  description = "External database URI (when managed=true)"
  type        = string
  default     = ""
  sensitive   = true
}

# Messaging
variable "messaging_type" {
  description = "Message broker type: rabbitmq or kafka"
  type        = string
  default     = "rabbitmq"
}

variable "messaging_managed" {
  description = "Use cloud-managed message broker"
  type        = bool
  default     = false
}

# Only needed if use_k8s_secrets = false
variable "mq_password" {
  description = "Message queue password (only used if use_k8s_secrets = false)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "mq_external_host" {
  description = "External broker host (when managed=true)"
  type        = string
  default     = ""
}

variable "mq_external_port" {
  description = "External broker port (when managed=true)"
  type        = number
  default     = 5672
}

variable "mq_external_uri" {
  description = "External broker URI (when managed=true)"
  type        = string
  default     = ""
  sensitive   = true
}

# Service mesh
variable "mesh_type" {
  description = "Service mesh type: linkerd or istio"
  type        = string
  default     = "linkerd"
}

variable "linkerd_trust_anchor_pem" {
  description = "Linkerd trust anchor certificate"
  type        = string
  sensitive   = true
}

variable "linkerd_issuer_cert_pem" {
  description = "Linkerd issuer certificate"
  type        = string
  sensitive   = true
}

variable "linkerd_issuer_key_pem" {
  description = "Linkerd issuer private key"
  type        = string
  sensitive   = true
}
