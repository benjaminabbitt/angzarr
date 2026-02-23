# RDS PostgreSQL Module - Outputs

locals {
  connection_uri = "postgres://${var.master_username}:${local.master_password}@${aws_db_instance.angzarr.address}:${aws_db_instance.angzarr.port}/${var.database_name}"
}

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities  = toset(["event_store", "position_store", "snapshot_store", "transactions"])
    cloud         = "aws"
    rust_features = toset(["postgres"])
    ha_mode       = var.multi_az ? "multi-az" : "single-az"
    secrets_backend = "aws"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null
    vpc             = true
    capabilities    = null
    secrets_backend = "aws"
  }
}

output "connection_uri" {
  description = "PostgreSQL connection URI"
  value       = local.connection_uri
  sensitive   = true
}

output "event_store" {
  description = "Event store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["event_store", "transactions"])
      rust_features = toset(["postgres"])
    }
  }
  sensitive = true
}

output "position_store" {
  description = "Position store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["position_store", "transactions"])
      rust_features = toset(["postgres"])
    }
  }
  sensitive = true
}

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["snapshot_store"])
      rust_features = toset(["postgres"])
    }
  }
  sensitive = true
}

output "endpoint" {
  description = "RDS endpoint"
  value       = aws_db_instance.angzarr.address
}

output "port" {
  description = "RDS port"
  value       = aws_db_instance.angzarr.port
}

output "instance_id" {
  description = "RDS instance ID"
  value       = aws_db_instance.angzarr.id
}

output "instance_arn" {
  description = "RDS instance ARN"
  value       = aws_db_instance.angzarr.arn
}

output "security_group_id" {
  description = "Security group ID for RDS"
  value       = aws_security_group.rds.id
}

output "secret_arn" {
  description = "Secrets Manager secret ARN"
  value       = aws_secretsmanager_secret.credentials.arn
}
