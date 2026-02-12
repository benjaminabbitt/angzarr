# Cloud SQL Module

Provisions a PostgreSQL instance on Google Cloud SQL for the angzarr event store.

## Overview

This module creates:
- Cloud SQL PostgreSQL instance
- Database and user
- Secret Manager secrets for credentials
- Optional private VPC networking

## Usage

### Basic Usage

```hcl
module "eventstore" {
  source = "../modules/cloudsql"

  project_id    = var.project_id
  region        = var.region
  instance_name = "angzarr-events"

  database = "angzarr"
  username = "angzarr"

  # Use default auto-generated password
  create_secrets   = true
  secret_accessors = [
    "serviceAccount:${module.order_aggregate.service_account_email}"
  ]
}
```

### Production Configuration

```hcl
module "eventstore" {
  source = "../modules/cloudsql"

  project_id    = var.project_id
  region        = var.region
  instance_name = "angzarr-events-prod"

  database = "angzarr"
  username = "angzarr"

  # Sizing
  tier              = "db-custom-2-4096"  # 2 vCPU, 4GB RAM
  availability_type = "REGIONAL"           # HA with failover
  disk_size         = 50
  disk_autoresize   = true

  # Networking - private only
  enable_public_ip  = false
  enable_private_ip = true
  vpc_network       = google_compute_network.vpc.self_link
  require_ssl       = true

  # Backup
  backup_enabled         = true
  point_in_time_recovery = true
  backup_retained_count  = 30

  # Protection
  deletion_protection = true

  # Monitoring
  query_insights_enabled = true

  create_secrets   = true
  secret_accessors = [
    "serviceAccount:${var.cloudrun_service_account}"
  ]
}
```

### With Cloud Run

```hcl
module "eventstore" {
  source = "../modules/cloudsql"
  # ...
}

module "order_aggregate" {
  source = "../modules/cloudrun-service"

  name              = "order-aggregate"
  coordinator_image = "gcr.io/${var.project_id}/angzarr-aggregate:latest"
  logic_image       = "gcr.io/${var.project_id}/agg-order:latest"

  # Use secret for DATABASE_URL
  coordinator_secrets = {
    DATABASE_URL = module.eventstore.cloudrun_secret_ref
  }

  # Cloud SQL Proxy connection (automatic with Cloud Run)
  coordinator_env = {
    STORAGE_TYPE = "postgres"
  }
}
```

## Connection Methods

### Cloud SQL Proxy (Recommended for Cloud Run)

Cloud Run has built-in Cloud SQL Proxy support. Use the `proxy_uri` output:

```hcl
# Connection string format:
# postgres://user:pass@localhost/db?host=/cloudsql/PROJECT:REGION:INSTANCE
```

### Private IP (VPC Connector)

For direct private IP connection, set up a VPC connector:

```hcl
resource "google_vpc_access_connector" "connector" {
  name          = "angzarr-connector"
  project       = var.project_id
  region        = var.region
  ip_cidr_range = "10.8.0.0/28"
  network       = google_compute_network.vpc.name
}

module "eventstore" {
  source = "../modules/cloudsql"
  # ...
  enable_private_ip = true
  vpc_network       = google_compute_network.vpc.self_link
}

# Use private_uri output
```

### Public IP (Development Only)

For development/debugging with authorized networks:

```hcl
module "eventstore" {
  source = "../modules/cloudsql"
  # ...
  enable_public_ip = true
  authorized_networks = [
    { name = "office", cidr = "203.0.113.0/24" }
  ]
}

# Use public_uri output
```

## Outputs

| Output | Description |
|--------|-------------|
| `instance_name` | Cloud SQL instance name |
| `connection_name` | Connection name for proxy (`project:region:instance`) |
| `private_ip` | Private IP address |
| `public_ip` | Public IP address |
| `database` | Database name |
| `username` | Database username |
| `password` | Database password (sensitive) |
| `proxy_uri` | URI for Cloud SQL Proxy |
| `private_uri` | URI for private IP connection |
| `public_uri` | URI for public IP connection |
| `password_secret_id` | Secret Manager ID for password |
| `uri_secret_id` | Secret Manager ID for URI |
| `cloudrun_secret_ref` | Ready-to-use secret ref for Cloud Run |

## Security

1. **Use private IP** in production - no public internet exposure
2. **Enable SSL** - `require_ssl = true`
3. **Use Secret Manager** - never pass credentials in plain text
4. **Limit access** - use `secret_accessors` to control who can read credentials
5. **Enable deletion protection** in production
