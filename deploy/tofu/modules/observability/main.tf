# Observability module - Grafana + Tempo + Prometheus + Loki + OTel Collector + Promtail
# Full observability stack for angzarr: traces, metrics, logs via OTLP
#
# Data flow:
#   Sidecars --OTLP--> OTel Collector ---> Tempo    (traces)
#                                     ---> Prometheus (metrics via remote write)
#                                     ---> Loki      (logs, structured via OTLP)
#   Promtail (DaemonSet) ---> Loki (stdout/stderr from business containers)
#   Grafana reads from all three backends.

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

# Namespace
resource "kubernetes_namespace" "monitoring" {
  count = var.create_namespace ? 1 : 0
  metadata {
    name = var.namespace
  }
}

locals {
  namespace = var.create_namespace ? kubernetes_namespace.monitoring[0].metadata[0].name : var.namespace

  # Internal service addresses (within cluster)
  tempo_endpoint      = "${var.release_prefix}-tempo.${local.namespace}.svc.cluster.local:4317"
  prometheus_endpoint = "http://${var.release_prefix}-prometheus-server.${local.namespace}.svc.cluster.local:80"
  loki_endpoint       = "http://${var.release_prefix}-loki.${local.namespace}.svc.cluster.local:3100"
}

# =============================================================================
# Tempo — Distributed tracing backend
# =============================================================================

resource "helm_release" "tempo" {
  name       = "${var.release_prefix}-tempo"
  repository = "https://grafana.github.io/helm-charts"
  chart      = "tempo"
  version    = var.tempo_chart_version
  namespace  = local.namespace

  values = [
    yamlencode({
      tempo = {
        # Single-binary mode for dev
        storage = {
          trace = {
            backend = "local"
            local = {
              path = "/var/tempo/traces"
            }
          }
        }
        # Accept OTLP gRPC from collector
        receivers = {
          otlp = {
            protocols = {
              grpc = {
                endpoint = "0.0.0.0:4317"
              }
            }
          }
        }
      }
      resources = {
        requests = {
          memory = var.resources.requests.memory
          cpu    = var.resources.requests.cpu
        }
        limits = {
          memory = var.resources.limits.memory
          cpu    = var.resources.limits.cpu
        }
      }
    })
  ]

  wait = true
}

# =============================================================================
# Prometheus — Metrics backend
# =============================================================================

resource "helm_release" "prometheus" {
  name       = "${var.release_prefix}-prometheus"
  repository = "https://prometheus-community.github.io/helm-charts"
  chart      = "prometheus"
  version    = var.prometheus_chart_version
  namespace  = local.namespace

  values = [
    yamlencode({
      server = {
        # Enable remote write receiver so OTel Collector can push metrics
        extraFlags = [
          "web.enable-remote-write-receiver",
        ]
        retention = "24h"
        persistentVolume = {
          enabled = false
        }
        resources = {
          requests = {
            memory = var.resources.requests.memory
            cpu    = var.resources.requests.cpu
          }
          limits = {
            memory = var.resources.limits.memory
            cpu    = var.resources.limits.cpu
          }
        }
      }
      # Disable components not needed for OTLP pipeline
      alertmanager = {
        enabled = false
      }
      kube-state-metrics = {
        enabled = false
      }
      prometheus-node-exporter = {
        enabled = false
      }
      prometheus-pushgateway = {
        enabled = false
      }
    })
  ]

  wait = true
}

# =============================================================================
# Loki — Log aggregation backend
# =============================================================================

resource "helm_release" "loki" {
  name       = "${var.release_prefix}-loki"
  repository = "https://grafana.github.io/helm-charts"
  chart      = "loki"
  version    = var.loki_chart_version
  namespace  = local.namespace

  values = [
    yamlencode({
      # Single-binary mode for dev
      deploymentMode = "SingleBinary"
      loki = {
        auth_enabled = false
        commonConfig = {
          replication_factor = 1
        }
        storage = {
          type = "filesystem"
        }
        schemaConfig = {
          configs = [
            {
              from         = "2024-01-01"
              store        = "tsdb"
              object_store = "filesystem"
              schema       = "v13"
              index = {
                prefix = "index_"
                period = "24h"
              }
            }
          ]
        }
      }
      singleBinary = {
        replicas = 1
        resources = {
          requests = {
            memory = var.resources.requests.memory
            cpu    = var.resources.requests.cpu
          }
          limits = {
            memory = var.resources.limits.memory
            cpu    = var.resources.limits.cpu
          }
        }
      }
      # Disable components not needed in single-binary mode
      backend = {
        replicas = 0
      }
      read = {
        replicas = 0
      }
      write = {
        replicas = 0
      }
      gateway = {
        enabled = false
      }
    })
  ]

  wait = true
}

# =============================================================================
# Promtail — DaemonSet log collector for container stdout/stderr
# =============================================================================
# Scrapes container logs from all pods and ships to Loki.
# Drops logs from containers named "angzarr" (sidecar) to avoid duplicating
# OTLP-exported logs. Only business application container logs are collected.

resource "helm_release" "promtail" {
  name       = "${var.release_prefix}-promtail"
  repository = "https://grafana.github.io/helm-charts"
  chart      = "promtail"
  version    = var.promtail_chart_version
  namespace  = local.namespace

  depends_on = [helm_release.loki]

  values = [
    yamlencode({
      config = {
        clients = [
          {
            url = "${local.loki_endpoint}/loki/api/v1/push"
          }
        ]
        snippets = {
          pipelineStages = [
            # Drop logs from angzarr sidecar containers — they export via OTLP
            {
              match = {
                selector            = "{container=\"angzarr\"}"
                action              = "drop"
                drop_counter_reason = "angzarr_sidecar_otlp_duplicate"
              }
            }
          ]
        }
      }
      resources = {
        requests = {
          memory = "64Mi"
          cpu    = "25m"
        }
        limits = {
          memory = "128Mi"
          cpu    = "100m"
        }
      }
    })
  ]

  wait = true
}

# =============================================================================
# OpenTelemetry Collector — Receives OTLP, fans out to backends
# =============================================================================

resource "helm_release" "otel_collector" {
  name       = "${var.release_prefix}-otel-collector"
  repository = "https://open-telemetry.github.io/opentelemetry-helm-charts"
  chart      = "opentelemetry-collector"
  version    = var.otel_collector_chart_version
  namespace  = local.namespace

  depends_on = [
    helm_release.tempo,
    helm_release.prometheus,
    helm_release.loki,
  ]

  values = [
    yamlencode({
      mode = "deployment"
      image = {
        repository = "otel/opentelemetry-collector-contrib"
      }
      config = {
        receivers = {
          otlp = {
            protocols = {
              grpc = {
                endpoint = "0.0.0.0:4317"
              }
              http = {
                endpoint = "0.0.0.0:4318"
              }
            }
          }
        }
        processors = {
          batch = {
            timeout         = "5s"
            send_batch_size = 1024
          }
          memory_limiter = {
            check_interval         = "1s"
            limit_percentage       = 80
            spike_limit_percentage = 25
          }
        }
        exporters = {
          # Traces -> Tempo via OTLP gRPC
          otlp = {
            endpoint = local.tempo_endpoint
            tls = {
              insecure = true
            }
          }
          # Metrics -> Prometheus via remote write
          prometheusremotewrite = {
            endpoint = "${local.prometheus_endpoint}/api/v1/write"
            tls = {
              insecure = true
            }
          }
          # Logs -> Loki via OTLP HTTP
          "otlphttp/loki" = {
            endpoint = "${local.loki_endpoint}/otlp"
            tls = {
              insecure = true
            }
          }
          debug = {
            verbosity = "basic"
          }
        }
        service = {
          pipelines = {
            traces = {
              receivers  = ["otlp"]
              processors = ["memory_limiter", "batch"]
              exporters  = ["otlp"]
            }
            metrics = {
              receivers  = ["otlp"]
              processors = ["memory_limiter", "batch"]
              exporters  = ["prometheusremotewrite"]
            }
            logs = {
              receivers  = ["otlp"]
              processors = ["memory_limiter", "batch"]
              exporters  = ["otlphttp/loki"]
            }
          }
        }
      }
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
          nodePort      = var.otel_collector_node_port
        }
        otlp-http = {
          enabled       = true
          containerPort = 4318
          servicePort   = 4318
          protocol      = "TCP"
        }
      }
      resources = {
        requests = {
          memory = var.resources.requests.memory
          cpu    = var.resources.requests.cpu
        }
        limits = {
          memory = var.resources.limits.memory
          cpu    = var.resources.limits.cpu
        }
      }
    })
  ]

  wait = true
}

# =============================================================================
# Grafana — Visualization
# =============================================================================

resource "helm_release" "grafana" {
  name       = "${var.release_prefix}-grafana"
  repository = "https://grafana.github.io/helm-charts"
  chart      = "grafana"
  version    = var.grafana_chart_version
  namespace  = local.namespace

  depends_on = [
    helm_release.tempo,
    helm_release.prometheus,
    helm_release.loki,
  ]

  values = [
    yamlencode({
      adminUser     = "admin"
      adminPassword = var.grafana_admin_password
      service = {
        type     = var.grafana_service_type
        nodePort = var.grafana_node_port
      }
      # Preconfigure datasources
      "grafana.ini" = {
        server = {
          root_url = "http://localhost:3000"
        }
      }
      # Plugins (auto-installed on startup)
      plugins = compact([
        var.topology_endpoint != "" ? "hamedkarbasi93-nodegraphapi-datasource" : "",
      ])
      # Sidecar watches ConfigMaps with grafana_dashboard=1 label
      sidecar = {
        dashboards = {
          enabled    = true
          label      = "grafana_dashboard"
          labelValue = "1"
          folder     = "/var/lib/grafana/dashboards/angzarr"
          provider = {
            name            = "angzarr"
            orgId           = 1
            folder          = "Angzarr"
            disableDeletion = false
            allowUiUpdates  = true
          }
        }
      }
      datasources = {
        "datasources.yaml" = {
          apiVersion = 1
          datasources = concat([
            {
              name      = "Tempo"
              type      = "tempo"
              access    = "proxy"
              url       = "http://${var.release_prefix}-tempo.${local.namespace}.svc.cluster.local:3100"
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
              url       = local.prometheus_endpoint
              isDefault = true
              uid       = "prometheus"
            },
            {
              name   = "Loki"
              type   = "loki"
              access = "proxy"
              url    = local.loki_endpoint
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
            ], var.topology_endpoint != "" ? [
            {
              name      = "Angzarr Topology"
              type      = "hamedkarbasi93-nodegraphapi-datasource"
              access    = "proxy"
              url       = var.topology_endpoint
              uid       = "topology"
              isDefault = false
              jsonData = {
                url = var.topology_endpoint
              }
            },
          ] : [])
        }
      }
      resources = {
        requests = {
          memory = var.resources.requests.memory
          cpu    = var.resources.requests.cpu
        }
        limits = {
          memory = var.resources.limits.memory
          cpu    = var.resources.limits.cpu
        }
      }
    })
  ]

  wait = true
}

# =============================================================================
# Grafana Dashboards — ConfigMaps picked up by sidecar
# =============================================================================

locals {
  dashboards_path = var.dashboards_path != "" ? var.dashboards_path : "${path.module}/../../../dashboards"
  dashboard_files = fileset(local.dashboards_path, "*.json")
}

resource "kubernetes_config_map" "dashboards" {
  for_each = local.dashboard_files

  metadata {
    name      = "angzarr-dashboard-${trimsuffix(each.value, ".json")}"
    namespace = local.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    (each.value) = file("${local.dashboards_path}/${each.value}")
  }

  depends_on = [helm_release.grafana]
}
