# Kubernetes Module - Outputs
# Standard interface for compute modules

output "provides" {
  description = "Capabilities provided by this compute module"
  value = {
    capabilities  = ["compute"]
    compute_type  = "kubernetes"
    cloud         = null # Works on any cloud or bare metal
    region        = null # Not cloud-specific
    ha_mode       = null # Depends on cluster configuration
    rust_features = []   # No Rust features required for compute
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null # This IS the compute
    vpc             = null # Not applicable
    capabilities    = null # No dependencies
    secrets_backend = null # Uses K8s secrets by default
  }
}

output "namespace" {
  description = "Kubernetes namespace for angzarr resources"
  value       = var.create_namespace ? kubernetes_namespace.angzarr[0].metadata[0].name : var.namespace
}

output "service_account_name" {
  description = "Service account name"
  value       = var.create_service_account ? kubernetes_service_account.angzarr[0].metadata[0].name : var.service_account_name
}

output "cluster_name" {
  description = "Cluster name (for identification)"
  value       = var.cluster_name
}

output "cluster_endpoint" {
  description = "Kubernetes API server endpoint"
  value       = var.cluster_endpoint
}
