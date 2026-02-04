# Local development environment
# Uses Helm charts for all infrastructure (no cloud-managed services)
# Reads credentials from K8s secrets (created by `just secrets-init`)
#
# Note: MongoDB, PostgreSQL, RabbitMQ, Kafka, and Redis are deployed via
# Helm subcharts (see values-rust.yaml) or externally in cloud environments.

terraform {
  required_version = ">= 1.0"

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

  # Local backend - ONLY for development
  # WARNING: Local state is not suitable for production or team environments.
  # For staging/prod, use a remote backend (S3, GCS, Azure, Terraform Cloud).
  # See deploy/terraform/README.md for configuration examples.
  backend "local" {
    path = "terraform.tfstate"
  }
}

provider "kubernetes" {
  config_path    = var.kubeconfig_path
  config_context = var.kubeconfig_context
}

provider "helm" {
  kubernetes {
    config_path    = var.kubeconfig_path
    config_context = var.kubeconfig_context
  }
}

# Read credentials from K8s secret (created by `just secrets-init`)
# This is the source of truth for all passwords
data "kubernetes_secret" "angzarr_secrets" {
  metadata {
    name      = "angzarr-secrets"
    namespace = var.secrets_namespace
  }
}

# Namespace for angzarr workloads
resource "kubernetes_namespace" "angzarr" {
  metadata {
    name = var.namespace
  }
}

# Observability - Grafana + Tempo + Prometheus + Loki + OTel Collector
resource "kubernetes_namespace" "monitoring" {
  count = var.enable_observability ? 1 : 0
  metadata {
    name = "monitoring"
  }
}

resource "helm_release" "observability" {
  count = var.enable_observability ? 1 : 0

  name       = "angzarr"
  chart      = "${path.module}/../../../helm/observability"
  namespace  = kubernetes_namespace.monitoring[0].metadata[0].name
  depends_on = [kubernetes_namespace.monitoring]

  values = [
    yamlencode({
      topologyEndpoint = "http://angzarr-topology.angzarr.svc.cluster.local:9099"

      grafana = {
        adminPassword = "angzarr"
        service = {
          type     = "NodePort"
          nodePort = 30300
        }
        # Add topology datasource
        datasources = {
          "datasources.yaml" = {
            apiVersion = 1
            datasources = [
              {
                name      = "Tempo"
                type      = "tempo"
                access    = "proxy"
                url       = "http://angzarr-tempo:3100"
                isDefault = false
                jsonData = {
                  tracesToLogsV2 = {
                    datasourceUid   = "loki"
                    filterByTraceID = true
                  }
                  tracesToMetrics = {
                    datasourceUid = "prometheus"
                  }
                }
              },
              {
                name      = "Prometheus"
                type      = "prometheus"
                access    = "proxy"
                url       = "http://angzarr-prometheus-server:80"
                isDefault = true
                uid       = "prometheus"
              },
              {
                name   = "Loki"
                type   = "loki"
                access = "proxy"
                url    = "http://angzarr-loki:3100"
                uid    = "loki"
                jsonData = {
                  derivedFields = [
                    {
                      datasourceUid = "tempo"
                      matcherRegex  = "trace_id=(\\w+)"
                      name          = "TraceID"
                      url           = "$${__value.raw}"
                    }
                  ]
                }
              },
              {
                name      = "Angzarr Topology"
                type      = "hamedkarbasi93-nodegraphapi-datasource"
                access    = "proxy"
                url       = "http://angzarr-topology.angzarr.svc.cluster.local:9099"
                uid       = "topology"
                isDefault = false
              },
            ]
          }
        }
      }

      "opentelemetry-collector" = {
        service = {
          type = "NodePort"
        }
        ports = {
          otlp = {
            enabled       = true
            containerPort = 4317
            servicePort   = 4317
            hostPort      = 4317
            protocol      = "TCP"
            nodePort      = 30417
          }
        }
      }
    })
  ]

  wait = true
}

# Service Mesh - Linkerd for local (lightweight, optional)
module "mesh" {
  count  = var.enable_mesh ? 1 : 0
  source = "../../modules/mesh"

  type             = "linkerd"
  namespace        = kubernetes_namespace.angzarr.metadata[0].name
  inject_namespace = true

  linkerd_trust_anchor_pem = var.linkerd_trust_anchor_pem
  linkerd_issuer_cert_pem  = var.linkerd_issuer_cert_pem
  linkerd_issuer_key_pem   = var.linkerd_issuer_key_pem

  proxy_resources = {
    requests = {
      memory = "32Mi"
      cpu    = "10m"
    }
    limits = {
      memory = "128Mi"
      cpu    = "500m"
    }
  }

  control_plane_resources = {
    requests = {
      memory = "128Mi"
      cpu    = "50m"
    }
    limits = {
      memory = "512Mi"
      cpu    = "500m"
    }
  }
}
