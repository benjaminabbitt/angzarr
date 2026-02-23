# MemoryDB Module - Outputs

locals {
  protocol       = var.tls_enabled ? "rediss" : "redis"
  connection_uri = "${local.protocol}://angzarr:${local.auth_token}@${aws_memorydb_cluster.angzarr.cluster_endpoint[0].address}:${aws_memorydb_cluster.angzarr.cluster_endpoint[0].port}"
}

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities  = toset(["snapshot_store", "caching"])
    cloud         = "aws"
    rust_features = toset(["redis"])
    ha_mode       = var.num_replicas_per_shard > 0 ? "multi-az" : "single-az"
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
  description = "Redis connection URI"
  value       = local.connection_uri
  sensitive   = true
}

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["snapshot_store"])
      rust_features = toset(["redis"])
    }
  }
  sensitive = true
}

output "cluster_endpoint" {
  description = "Cluster endpoint address"
  value       = aws_memorydb_cluster.angzarr.cluster_endpoint[0].address
}

output "cluster_port" {
  description = "Cluster endpoint port"
  value       = aws_memorydb_cluster.angzarr.cluster_endpoint[0].port
}

output "cluster_arn" {
  description = "Cluster ARN"
  value       = aws_memorydb_cluster.angzarr.arn
}

output "security_group_id" {
  description = "Security group ID"
  value       = aws_security_group.memorydb.id
}

output "secret_arn" {
  description = "Secrets Manager secret ARN"
  value       = aws_secretsmanager_secret.credentials.arn
}
