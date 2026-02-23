# Kubernetes Module - Variables
# Configuration for wrapping any Kubernetes cluster

variable "namespace" {
  description = "Kubernetes namespace for angzarr resources"
  type        = string
  default     = "angzarr"
}

variable "create_namespace" {
  description = "Create the namespace (set false if it already exists)"
  type        = bool
  default     = true
}

variable "service_account_name" {
  description = "Service account name for angzarr workloads"
  type        = string
  default     = "angzarr"
}

variable "create_service_account" {
  description = "Create the service account (set false if using existing or cloud-provided)"
  type        = bool
  default     = true
}

variable "service_account_annotations" {
  description = <<-EOT
    Annotations for the service account.

    For GKE Workload Identity:
      { "iam.gke.io/gcp-service-account" = "sa@project.iam.gserviceaccount.com" }

    For EKS IRSA:
      { "eks.amazonaws.com/role-arn" = "arn:aws:iam::123456789:role/role-name" }
  EOT
  type        = map(string)
  default     = {}
}

variable "create_rbac" {
  description = "Create RBAC resources (ClusterRole, ClusterRoleBinding)"
  type        = bool
  default     = true
}

variable "labels" {
  description = "Labels to apply to all resources"
  type        = map(string)
  default = {
    "app.kubernetes.io/managed-by" = "angzarr"
  }
}

#------------------------------------------------------------------------------
# Cluster Information (for outputs)
#------------------------------------------------------------------------------

variable "cluster_name" {
  description = "Name of the Kubernetes cluster (for identification/tagging)"
  type        = string
  default     = "default"
}

variable "cluster_endpoint" {
  description = "Kubernetes API server endpoint (optional, for reference)"
  type        = string
  default     = null
}
