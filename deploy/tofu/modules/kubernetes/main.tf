# Kubernetes Module - Main
# Wrapper for any Kubernetes cluster (Kind, GKE, EKS, AKS, OpenShift, Rancher, Tanzu, self-hosted)
#
# This module does not create a cluster - it wraps an existing cluster's connection
# and provides the standard interface expected by domain modules.

terraform {
  required_version = ">= 1.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.0"
    }
  }
}

#------------------------------------------------------------------------------
# Namespace (optional)
#------------------------------------------------------------------------------

resource "kubernetes_namespace" "angzarr" {
  count = var.create_namespace ? 1 : 0

  metadata {
    name   = var.namespace
    labels = var.labels
  }
}

#------------------------------------------------------------------------------
# Service Account (optional)
#------------------------------------------------------------------------------

resource "kubernetes_service_account" "angzarr" {
  count = var.create_service_account ? 1 : 0

  metadata {
    name      = var.service_account_name
    namespace = var.create_namespace ? kubernetes_namespace.angzarr[0].metadata[0].name : var.namespace
    labels    = var.labels

    annotations = var.service_account_annotations
  }
}

#------------------------------------------------------------------------------
# RBAC (optional)
#------------------------------------------------------------------------------

resource "kubernetes_cluster_role" "angzarr" {
  count = var.create_rbac ? 1 : 0

  metadata {
    name   = "angzarr-${var.namespace}"
    labels = var.labels
  }

  # ConfigMaps for coordination
  rule {
    api_groups = [""]
    resources  = ["configmaps"]
    verbs      = ["get", "list", "watch", "create", "update", "patch", "delete"]
  }

  # Secrets for credentials
  rule {
    api_groups = [""]
    resources  = ["secrets"]
    verbs      = ["get", "list", "watch"]
  }

  # Events for debugging
  rule {
    api_groups = [""]
    resources  = ["events"]
    verbs      = ["create", "patch"]
  }
}

resource "kubernetes_cluster_role_binding" "angzarr" {
  count = var.create_rbac ? 1 : 0

  metadata {
    name   = "angzarr-${var.namespace}"
    labels = var.labels
  }

  role_ref {
    api_group = "rbac.authorization.k8s.io"
    kind      = "ClusterRole"
    name      = kubernetes_cluster_role.angzarr[0].metadata[0].name
  }

  subject {
    kind      = "ServiceAccount"
    name      = var.create_service_account ? kubernetes_service_account.angzarr[0].metadata[0].name : var.service_account_name
    namespace = var.create_namespace ? kubernetes_namespace.angzarr[0].metadata[0].name : var.namespace
  }
}
