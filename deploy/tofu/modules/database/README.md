# Database Module

Deploys PostgreSQL or MongoDB via Bitnami Helm charts for Angzarr event storage.

## Usage

```hcl
module "database" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/database?ref=v0.1.0"

  type         = "mongodb"  # or "postgresql"
  namespace    = "angzarr"
  release_name = "angzarr-db"

  # Optional: Provide passwords or let module auto-generate
  # admin_password = var.db_admin_password
  # password       = var.db_password
}
```

## Inputs

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|----------|
| type | Database type: `postgresql` or `mongodb` | string | - | yes |
| managed | Use cloud-managed database instead of Helm | bool | `false` | no |
| release_name | Helm release name | string | `"angzarr-db"` | no |
| namespace | Kubernetes namespace | string | `"angzarr"` | no |
| admin_password | Admin/root password (auto-generated if null) | string | `null` | no |
| username | Application database username | string | `"angzarr"` | no |
| password | Application password (auto-generated if null) | string | `null` | no |
| database | Database name | string | `"angzarr"` | no |
| persistence_enabled | Enable persistent storage | bool | `true` | no |
| persistence_size | Persistent volume size | string | `"8Gi"` | no |
| metrics_enabled | Enable Prometheus metrics | bool | `true` | no |

## Outputs

| Name | Description |
|------|-------------|
| host | Database host |
| port | Database port |
| uri | Connection URI (sensitive) |
| username | Database username |
| password | Database password (sensitive) |
| admin_password | Admin password (sensitive) |
| secret_name | Kubernetes secret name |
| type | Database type |

## External/Managed Databases

For cloud-managed databases (RDS, Atlas, etc.), set `managed = true` and provide connection details:

```hcl
module "database" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/database?ref=v0.1.0"

  type    = "postgresql"
  managed = true

  external_host = "my-db.xxx.rds.amazonaws.com"
  external_port = 5432
  external_uri  = "postgres://user:pass@my-db.xxx.rds.amazonaws.com:5432/angzarr"
}
```

## Requirements

| Name | Version |
|------|---------|
| terraform/opentofu | >= 1.0 |
| helm | ~> 2.0 |
| kubernetes | ~> 2.0 |
| random | ~> 3.0 |
