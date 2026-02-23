# Cloud Run Base Module - Main
# GCP Cloud Run compute base (service account, VPC connector)

terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = ">= 5.0"
    }
  }
}

locals {
  labels = merge(var.labels, {
    "angzarr-component" = "compute"
    "angzarr-compute"   = "cloudrun"
  })
}

#------------------------------------------------------------------------------
# Service Account for Cloud Run Services
#------------------------------------------------------------------------------

resource "google_service_account" "cloudrun" {
  account_id   = "${var.name_prefix}-cloudrun"
  project      = var.project_id
  display_name = "Cloud Run Service Account for ${var.name_prefix}"
}

# Allow invoking Cloud Run services (for internal communication)
resource "google_project_iam_member" "cloudrun_invoker" {
  project = var.project_id
  role    = "roles/run.invoker"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

# Allow reading secrets
resource "google_project_iam_member" "secret_accessor" {
  project = var.project_id
  role    = "roles/secretmanager.secretAccessor"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

# Allow writing logs
resource "google_project_iam_member" "log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

# Allow writing metrics
resource "google_project_iam_member" "metric_writer" {
  project = var.project_id
  role    = "roles/monitoring.metricWriter"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

# Allow writing traces
resource "google_project_iam_member" "trace_agent" {
  project = var.project_id
  role    = "roles/cloudtrace.agent"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

#------------------------------------------------------------------------------
# Serverless VPC Access Connector (optional)
#------------------------------------------------------------------------------

resource "google_vpc_access_connector" "cloudrun" {
  count = var.create_vpc_connector ? 1 : 0

  name          = "${var.name_prefix}-connector"
  project       = var.project_id
  region        = var.region
  network       = var.vpc_connector_network
  ip_cidr_range = var.vpc_connector_ip_range

  min_instances = var.vpc_connector_min_instances
  max_instances = var.vpc_connector_max_instances

  machine_type = var.vpc_connector_machine_type
}

#------------------------------------------------------------------------------
# Additional IAM bindings (optional)
#------------------------------------------------------------------------------

resource "google_project_iam_member" "additional" {
  for_each = toset(var.additional_roles)

  project = var.project_id
  role    = each.value
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}
