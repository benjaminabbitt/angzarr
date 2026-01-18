# Service Mesh module - Linkerd or Istio via Helm
# Provides L7 gRPC load balancing for angzarr services

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
  }
}

# Linkerd CRDs (required before linkerd-control-plane)
resource "helm_release" "linkerd_crds" {
  count = var.type == "linkerd" ? 1 : 0

  name       = "linkerd-crds"
  repository = "https://helm.linkerd.io/stable"
  chart      = "linkerd-crds"
  version    = var.linkerd_chart_version
  namespace  = "linkerd"

  create_namespace = true
  wait             = true
}

# Linkerd Control Plane
resource "helm_release" "linkerd" {
  count = var.type == "linkerd" ? 1 : 0

  depends_on = [helm_release.linkerd_crds]

  name       = "linkerd-control-plane"
  repository = "https://helm.linkerd.io/stable"
  chart      = "linkerd-control-plane"
  version    = var.linkerd_chart_version
  namespace  = "linkerd"

  values = [
    yamlencode({
      identityTrustAnchorsPEM = var.linkerd_trust_anchor_pem
      identity = {
        issuer = {
          tls = {
            crtPEM = var.linkerd_issuer_cert_pem
            keyPEM = var.linkerd_issuer_key_pem
          }
        }
      }
      proxy = {
        resources = {
          cpu = {
            request = var.proxy_resources.requests.cpu
            limit   = var.proxy_resources.limits.cpu
          }
          memory = {
            request = var.proxy_resources.requests.memory
            limit   = var.proxy_resources.limits.memory
          }
        }
      }
      # Enable gRPC load balancing
      proxyInit = {
        runAsRoot = var.linkerd_run_as_root
      }
    })
  ]

  wait = true
}

# Istio Base (CRDs)
resource "helm_release" "istio_base" {
  count = var.type == "istio" ? 1 : 0

  name       = "istio-base"
  repository = "https://istio-release.storage.googleapis.com/charts"
  chart      = "base"
  version    = var.istio_chart_version
  namespace  = "istio-system"

  create_namespace = true
  wait             = true
}

# Istio Discovery (istiod)
resource "helm_release" "istiod" {
  count = var.type == "istio" ? 1 : 0

  depends_on = [helm_release.istio_base]

  name       = "istiod"
  repository = "https://istio-release.storage.googleapis.com/charts"
  chart      = "istiod"
  version    = var.istio_chart_version
  namespace  = "istio-system"

  values = [
    yamlencode({
      pilot = {
        resources = {
          requests = {
            cpu    = var.control_plane_resources.requests.cpu
            memory = var.control_plane_resources.requests.memory
          }
          limits = {
            cpu    = var.control_plane_resources.limits.cpu
            memory = var.control_plane_resources.limits.memory
          }
        }
      }
      global = {
        proxy = {
          resources = {
            requests = {
              cpu    = var.proxy_resources.requests.cpu
              memory = var.proxy_resources.requests.memory
            }
            limits = {
              cpu    = var.proxy_resources.limits.cpu
              memory = var.proxy_resources.limits.memory
            }
          }
        }
      }
      meshConfig = {
        # Enable strict mTLS
        defaultConfig = {
          holdApplicationUntilProxyStarts = true
        }
      }
    })
  ]

  wait = true
}

# Annotate angzarr namespace for mesh injection
resource "kubernetes_namespace" "angzarr" {
  count = var.inject_namespace ? 1 : 0

  metadata {
    name = var.namespace

    annotations = var.type == "linkerd" ? {
      "linkerd.io/inject" = "enabled"
    } : {}

    labels = var.type == "istio" ? {
      "istio-injection" = "enabled"
    } : {}
  }
}
