# Domain Module
# Deploys all components for a single domain:
# - Aggregate or Process Manager (with optional upcaster sidecar)
# - Sagas (that source from this domain)
# - Projectors (that source from this domain)

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

check "aggregate_pm_exclusive" {
  assert {
    condition     = !(var.aggregate.enabled && var.process_manager.enabled)
    error_message = "aggregate and process_manager are mutually exclusive. A domain can be either an aggregate (command handler) or a process manager (cross-domain orchestrator), not both."
  }
}

locals {
  labels = merge(
    {
      "angzarr-domain" = var.domain
      "managed-by"     = "opentofu"
    },
    var.labels
  )

  # Coordinator env shared by all components
  base_coordinator_env = merge(
    {
      "RUST_LOG"       = var.log_level
      "TRANSPORT_TYPE" = "tcp"
      "DOMAIN"         = var.domain
    },
    var.discovery_env,
    var.coordinator_env
  )

  # Port configuration
  ports = {
    grpc_gateway = 8080
    coordinator  = 1310
    logic        = 50053
    upcaster     = 50054
  }

  # Default scaling configs for sagas and projectors
  default_saga_scaling = {
    min_instances = 0
    max_instances = 10
    resources = {
      cpu    = "1"
      memory = "256Mi"
    }
  }

  default_projector_scaling = {
    min_instances = 0
    max_instances = 10
    resources = {
      cpu    = "1"
      memory = "256Mi"
    }
  }

  # Per-saga scaling with defaults
  saga_scaling = {
    for name, _ in var.sagas :
    name => merge(local.default_saga_scaling, try(var.scaling.sagas[name], {}))
  }

  # Per-projector scaling with defaults
  projector_scaling = {
    for name, _ in var.projectors :
    name => merge(local.default_projector_scaling, try(var.scaling.projectors[name], {}))
  }
}

#------------------------------------------------------------------------------
# Service Account
#------------------------------------------------------------------------------
resource "google_service_account" "domain" {
  count = var.iam.create_service_account ? 1 : 0

  account_id   = "${var.domain}-domain-sa"
  display_name = "Service account for ${var.domain} domain"
  project      = var.project_id
}

locals {
  service_account = var.iam.create_service_account ? google_service_account.domain[0].email : var.iam.service_account
}

#------------------------------------------------------------------------------
# Aggregate Service
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service" "aggregate" {
  count = var.aggregate.enabled ? 1 : 0

  name     = "${var.domain}-aggregate"
  location = var.region
  project  = var.project_id
  labels   = merge(local.labels, { "angzarr-component" = "aggregate" })

  template {
    labels          = merge(local.labels, { "angzarr-component" = "aggregate" })
    service_account = local.service_account

    scaling {
      min_instance_count = var.scaling.aggregate.min_instances
      max_instance_count = var.scaling.aggregate.max_instances
    }

    execution_environment = var.execution.environment
    timeout               = "${var.execution.timeout_seconds}s"
    session_affinity      = true

    dynamic "vpc_access" {
      for_each = var.networking.vpc_connector != null ? [1] : []
      content {
        connector = var.networking.vpc_connector
        egress    = var.networking.vpc_egress
      }
    }

    # grpc-gateway container (REST bridge, exposed)
    containers {
      name  = "grpc-gateway"
      image = var.images.grpc_gateway

      ports {
        name           = "http1"
        container_port = local.ports.grpc_gateway
      }

      resources {
        limits = {
          cpu    = var.scaling.grpc_gateway.resources.cpu
          memory = var.scaling.grpc_gateway.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      env {
        name  = "GRPC_BACKEND"
        value = "localhost:${local.ports.coordinator}"
      }
      env {
        name  = "PORT"
        value = tostring(local.ports.grpc_gateway)
      }

      startup_probe {
        http_get {
          path = "/health"
          port = local.ports.grpc_gateway
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # Coordinator container
    containers {
      name  = "coordinator"
      image = var.images.coordinator_aggregate

      resources {
        limits = {
          cpu    = var.scaling.coordinator.resources.cpu
          memory = var.scaling.coordinator.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(local.base_coordinator_env, {
          "PORT"                       = tostring(local.ports.coordinator)
          "COMPONENT_TYPE"             = "aggregate"
          "ANGZARR__TARGET__ADDRESS"   = "localhost:${local.ports.logic}"
          "ANGZARR_UPCASTER_ENABLED"   = tostring(var.aggregate.upcaster.enabled)
          "ANGZARR_UPCASTER_ADDRESS"   = var.aggregate.upcaster.enabled ? "localhost:${local.ports.upcaster}" : ""
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = var.coordinator_secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      startup_probe {
        grpc {
          port = local.ports.coordinator
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.coordinator
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = var.images.logic

      resources {
        limits = {
          cpu    = var.scaling.aggregate.resources.cpu
          memory = var.scaling.aggregate.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(var.aggregate.env, {
          "RUST_LOG" = var.log_level
          "PORT"     = tostring(local.ports.logic)
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      startup_probe {
        grpc {
          port = local.ports.logic
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.logic
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }

    # Optional upcaster container
    dynamic "containers" {
      for_each = var.aggregate.upcaster.enabled ? [1] : []
      content {
        name  = "upcaster"
        image = var.images.upcaster

        resources {
          limits = {
            cpu    = var.scaling.upcaster.resources.cpu
            memory = var.scaling.upcaster.resources.memory
          }
          cpu_idle = var.execution.cpu_idle
        }

        dynamic "env" {
          for_each = merge(var.aggregate.upcaster.env, {
            "RUST_LOG" = var.log_level
            "PORT"     = tostring(local.ports.upcaster)
            "DOMAIN"   = var.domain
          })
          content {
            name  = env.key
            value = env.value
          }
        }

        startup_probe {
          grpc {
            port = local.ports.upcaster
          }
          initial_delay_seconds = 5
          period_seconds        = 10
          failure_threshold     = 3
        }
      }
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

#------------------------------------------------------------------------------
# Process Manager Service
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service" "process_manager" {
  count = var.process_manager.enabled ? 1 : 0

  name     = "${var.domain}-pm"
  location = var.region
  project  = var.project_id
  labels   = merge(local.labels, { "angzarr-component" = "process-manager" })

  template {
    labels          = merge(local.labels, { "angzarr-component" = "process-manager" })
    service_account = local.service_account

    scaling {
      min_instance_count = var.scaling.process_manager.min_instances
      max_instance_count = var.scaling.process_manager.max_instances
    }

    execution_environment = var.execution.environment
    timeout               = "${var.execution.timeout_seconds}s"

    dynamic "vpc_access" {
      for_each = var.networking.vpc_connector != null ? [1] : []
      content {
        connector = var.networking.vpc_connector
        egress    = var.networking.vpc_egress
      }
    }

    # grpc-gateway container (REST bridge, exposed)
    containers {
      name  = "grpc-gateway"
      image = var.images.grpc_gateway

      ports {
        name           = "http1"
        container_port = local.ports.grpc_gateway
      }

      resources {
        limits = {
          cpu    = var.scaling.grpc_gateway.resources.cpu
          memory = var.scaling.grpc_gateway.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      env {
        name  = "GRPC_BACKEND"
        value = "localhost:${local.ports.coordinator}"
      }
      env {
        name  = "PORT"
        value = tostring(local.ports.grpc_gateway)
      }

      startup_probe {
        http_get {
          path = "/health"
          port = local.ports.grpc_gateway
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # Coordinator container
    containers {
      name  = "coordinator"
      image = var.images.coordinator_pm

      resources {
        limits = {
          cpu    = var.scaling.coordinator.resources.cpu
          memory = var.scaling.coordinator.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(local.base_coordinator_env, {
          "PORT"                     = tostring(local.ports.coordinator)
          "COMPONENT_TYPE"           = "process_manager"
          "ANGZARR__TARGET__ADDRESS" = "localhost:${local.ports.logic}"
          "SOURCE_DOMAINS"           = join(",", var.process_manager.source_domains)
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = var.coordinator_secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      startup_probe {
        grpc {
          port = local.ports.coordinator
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.coordinator
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = var.images.logic

      resources {
        limits = {
          cpu    = var.scaling.process_manager.resources.cpu
          memory = var.scaling.process_manager.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(var.process_manager.env, {
          "RUST_LOG" = var.log_level
          "PORT"     = tostring(local.ports.logic)
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      startup_probe {
        grpc {
          port = local.ports.logic
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.logic
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

#------------------------------------------------------------------------------
# Saga Services
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service" "saga" {
  for_each = var.sagas

  name     = "saga-${var.domain}-${each.key}"
  location = var.region
  project  = var.project_id
  labels = merge(local.labels, {
    "angzarr-component"     = "saga"
    "angzarr-target-domain" = each.value.target_domain
  })

  template {
    labels = merge(local.labels, {
      "angzarr-component"     = "saga"
      "angzarr-target-domain" = each.value.target_domain
    })
    service_account = local.service_account

    scaling {
      min_instance_count = local.saga_scaling[each.key].min_instances
      max_instance_count = local.saga_scaling[each.key].max_instances
    }

    execution_environment = var.execution.environment
    timeout               = "${var.execution.timeout_seconds}s"

    dynamic "vpc_access" {
      for_each = var.networking.vpc_connector != null ? [1] : []
      content {
        connector = var.networking.vpc_connector
        egress    = var.networking.vpc_egress
      }
    }

    # grpc-gateway container
    containers {
      name  = "grpc-gateway"
      image = var.images.grpc_gateway

      ports {
        name           = "http1"
        container_port = local.ports.grpc_gateway
      }

      resources {
        limits = {
          cpu    = var.scaling.grpc_gateway.resources.cpu
          memory = var.scaling.grpc_gateway.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      env {
        name  = "GRPC_BACKEND"
        value = "localhost:${local.ports.coordinator}"
      }
      env {
        name  = "PORT"
        value = tostring(local.ports.grpc_gateway)
      }

      startup_probe {
        http_get {
          path = "/health"
          port = local.ports.grpc_gateway
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # Coordinator container
    containers {
      name  = "coordinator"
      image = var.images.coordinator_saga

      resources {
        limits = {
          cpu    = var.scaling.coordinator.resources.cpu
          memory = var.scaling.coordinator.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(local.base_coordinator_env, {
          "PORT"                     = tostring(local.ports.coordinator)
          "COMPONENT_TYPE"           = "saga"
          "ANGZARR__TARGET__ADDRESS" = "localhost:${local.ports.logic}"
          "TARGET_DOMAIN"            = each.value.target_domain
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = var.coordinator_secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      startup_probe {
        grpc {
          port = local.ports.coordinator
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.coordinator
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = lookup(var.images.saga_logic, each.key, var.images.logic)

      resources {
        limits = {
          cpu    = local.saga_scaling[each.key].resources.cpu
          memory = local.saga_scaling[each.key].resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(each.value.env, {
          "RUST_LOG" = var.log_level
          "PORT"     = tostring(local.ports.logic)
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      startup_probe {
        grpc {
          port = local.ports.logic
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.logic
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

#------------------------------------------------------------------------------
# Projector Services
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service" "projector" {
  for_each = var.projectors

  name     = "projector-${var.domain}-${each.key}"
  location = var.region
  project  = var.project_id
  labels = merge(local.labels, {
    "angzarr-component" = "projector"
    "projector-name"    = each.key
  })

  template {
    labels = merge(local.labels, {
      "angzarr-component" = "projector"
      "projector-name"    = each.key
    })
    service_account = local.service_account

    scaling {
      min_instance_count = local.projector_scaling[each.key].min_instances
      max_instance_count = local.projector_scaling[each.key].max_instances
    }

    execution_environment = var.execution.environment
    timeout               = "${var.execution.timeout_seconds}s"

    dynamic "vpc_access" {
      for_each = var.networking.vpc_connector != null ? [1] : []
      content {
        connector = var.networking.vpc_connector
        egress    = var.networking.vpc_egress
      }
    }

    # grpc-gateway container
    containers {
      name  = "grpc-gateway"
      image = var.images.grpc_gateway

      ports {
        name           = "http1"
        container_port = local.ports.grpc_gateway
      }

      resources {
        limits = {
          cpu    = var.scaling.grpc_gateway.resources.cpu
          memory = var.scaling.grpc_gateway.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      env {
        name  = "GRPC_BACKEND"
        value = "localhost:${local.ports.coordinator}"
      }
      env {
        name  = "PORT"
        value = tostring(local.ports.grpc_gateway)
      }

      startup_probe {
        http_get {
          path = "/health"
          port = local.ports.grpc_gateway
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }
    }

    # Coordinator container
    containers {
      name  = "coordinator"
      image = var.images.coordinator_projector

      resources {
        limits = {
          cpu    = var.scaling.coordinator.resources.cpu
          memory = var.scaling.coordinator.resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(local.base_coordinator_env, {
          "PORT"                     = tostring(local.ports.coordinator)
          "COMPONENT_TYPE"           = "projector"
          "ANGZARR__TARGET__ADDRESS" = "localhost:${local.ports.logic}"
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = var.coordinator_secrets
        content {
          name = env.key
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }

      startup_probe {
        grpc {
          port = local.ports.coordinator
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.coordinator
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }

    # Business logic container
    containers {
      name  = "logic"
      image = lookup(var.images.projector_logic, each.key, var.images.logic)

      resources {
        limits = {
          cpu    = local.projector_scaling[each.key].resources.cpu
          memory = local.projector_scaling[each.key].resources.memory
        }
        cpu_idle          = var.execution.cpu_idle
        startup_cpu_boost = true
      }

      dynamic "env" {
        for_each = merge(each.value.env, {
          "RUST_LOG" = var.log_level
          "PORT"     = tostring(local.ports.logic)
        })
        content {
          name  = env.key
          value = env.value
        }
      }

      startup_probe {
        grpc {
          port = local.ports.logic
        }
        initial_delay_seconds = 5
        period_seconds        = 10
        failure_threshold     = 3
      }

      liveness_probe {
        grpc {
          port = local.ports.logic
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }
  }

  traffic {
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
    percent = 100
  }
}

#------------------------------------------------------------------------------
# IAM - Allow unauthenticated access if specified
#------------------------------------------------------------------------------
resource "google_cloud_run_v2_service_iam_member" "aggregate_public" {
  count = var.aggregate.enabled && var.iam.allow_unauthenticated ? 1 : 0

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.aggregate[0].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

resource "google_cloud_run_v2_service_iam_member" "pm_public" {
  count = var.process_manager.enabled && var.iam.allow_unauthenticated ? 1 : 0

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.process_manager[0].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

resource "google_cloud_run_v2_service_iam_member" "saga_public" {
  for_each = var.iam.allow_unauthenticated ? var.sagas : {}

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.saga[each.key].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

resource "google_cloud_run_v2_service_iam_member" "projector_public" {
  for_each = var.iam.allow_unauthenticated ? var.projectors : {}

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.projector[each.key].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}
