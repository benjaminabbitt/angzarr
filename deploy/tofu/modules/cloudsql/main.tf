# Cloud SQL Module
# Provisions PostgreSQL instance for angzarr event store on GCP
#
# Creates:
# - Cloud SQL instance (PostgreSQL)
# - Database and user
# - Secret Manager secrets for credentials
# - Private VPC peering (optional)

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }
}

# Generate password if not provided
resource "random_password" "db_password" {
  count   = var.password == null ? 1 : 0
  length  = 32
  special = false
}

locals {
  password = var.password != null ? var.password : random_password.db_password[0].result

  # Connection string formats
  public_connection_name = google_sql_database_instance.instance.connection_name
  private_ip             = var.enable_private_ip ? google_sql_database_instance.instance.private_ip_address : null
  public_ip              = var.enable_public_ip ? google_sql_database_instance.instance.public_ip_address : null

  # URI for different connection methods
  # Cloud Run uses Cloud SQL Proxy via connection name
  proxy_uri = "postgres://${var.username}:${local.password}@localhost/${var.database}?host=/cloudsql/${local.public_connection_name}"

  # Direct private IP connection (requires VPC connector)
  private_uri = local.private_ip != null ? "postgres://${var.username}:${local.password}@${local.private_ip}:5432/${var.database}" : null

  # Direct public IP connection (requires authorized networks)
  public_uri = local.public_ip != null ? "postgres://${var.username}:${local.password}@${local.public_ip}:5432/${var.database}" : null

  labels = merge(
    {
      "managed-by" = "opentofu"
      "component"  = "event-store"
    },
    var.labels
  )
}

# Cloud SQL instance
resource "google_sql_database_instance" "instance" {
  name             = var.instance_name
  project          = var.project_id
  region           = var.region
  database_version = var.database_version

  deletion_protection = var.deletion_protection

  settings {
    tier              = var.tier
    availability_type = var.availability_type
    disk_type         = var.disk_type
    disk_size         = var.disk_size
    disk_autoresize   = var.disk_autoresize

    user_labels = local.labels

    # Backup configuration
    backup_configuration {
      enabled                        = var.backup_enabled
      start_time                     = var.backup_start_time
      point_in_time_recovery_enabled = var.point_in_time_recovery
      backup_retention_settings {
        retained_backups = var.backup_retained_count
      }
    }

    # Maintenance window
    maintenance_window {
      day          = var.maintenance_window_day
      hour         = var.maintenance_window_hour
      update_track = var.maintenance_update_track
    }

    # IP configuration
    ip_configuration {
      ipv4_enabled    = var.enable_public_ip
      private_network = var.enable_private_ip ? var.vpc_network : null
      require_ssl     = var.require_ssl

      # Authorized networks for public IP access
      dynamic "authorized_networks" {
        for_each = var.authorized_networks
        content {
          name  = authorized_networks.value.name
          value = authorized_networks.value.cidr
        }
      }
    }

    # Insights for query performance
    insights_config {
      query_insights_enabled  = var.query_insights_enabled
      query_plans_per_minute  = var.query_insights_enabled ? 5 : 0
      query_string_length     = var.query_insights_enabled ? 1024 : 0
      record_application_tags = var.query_insights_enabled
      record_client_address   = var.query_insights_enabled
    }

    # Database flags
    dynamic "database_flags" {
      for_each = var.database_flags
      content {
        name  = database_flags.key
        value = database_flags.value
      }
    }
  }

  lifecycle {
    prevent_destroy = false
  }
}

# Database
resource "google_sql_database" "database" {
  name     = var.database
  instance = google_sql_database_instance.instance.name
  project  = var.project_id
}

# User
resource "google_sql_user" "user" {
  name     = var.username
  instance = google_sql_database_instance.instance.name
  project  = var.project_id
  password = local.password
}

# Store credentials in Secret Manager
resource "google_secret_manager_secret" "db_password" {
  count = var.create_secrets ? 1 : 0

  secret_id = "${var.instance_name}-password"
  project   = var.project_id

  labels = local.labels

  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "db_password" {
  count = var.create_secrets ? 1 : 0

  secret      = google_secret_manager_secret.db_password[0].id
  secret_data = local.password
}

resource "google_secret_manager_secret" "db_uri" {
  count = var.create_secrets ? 1 : 0

  secret_id = "${var.instance_name}-uri"
  project   = var.project_id

  labels = local.labels

  replication {
    auto {}
  }
}

resource "google_secret_manager_secret_version" "db_uri" {
  count = var.create_secrets ? 1 : 0

  secret      = google_secret_manager_secret.db_uri[0].id
  secret_data = local.proxy_uri
}

# IAM binding for Cloud Run to access secrets
resource "google_secret_manager_secret_iam_member" "password_accessor" {
  for_each = var.create_secrets ? toset(var.secret_accessors) : toset([])

  project   = var.project_id
  secret_id = google_secret_manager_secret.db_password[0].secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = each.value
}

resource "google_secret_manager_secret_iam_member" "uri_accessor" {
  for_each = var.create_secrets ? toset(var.secret_accessors) : toset([])

  project   = var.project_id
  secret_id = google_secret_manager_secret.db_uri[0].secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = each.value
}
