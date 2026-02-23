# DynamoDB Module - Outputs

locals {
  connection_uri = "dynamodb://${var.table_prefix}"
}

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities  = toset(["event_store", "position_store", "snapshot_store", "transactions"])
    cloud         = "aws"
    rust_features = toset(["dynamo"])
    ha_mode       = "multi-az" # DynamoDB is inherently multi-AZ
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null  # Works with any compute
    vpc             = false # No VPC required (uses AWS APIs)
    capabilities    = null
    secrets_backend = null
  }
}

output "connection_uri" {
  description = "Connection URI for coordinators"
  value       = local.connection_uri
}

output "event_store" {
  description = "Event store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["event_store", "transactions"])
      rust_features = toset(["dynamo"])
    }
  }
}

output "position_store" {
  description = "Position store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["position_store", "transactions"])
      rust_features = toset(["dynamo"])
    }
  }
}

output "snapshot_store" {
  description = "Snapshot store configuration for stack module"
  value = {
    connection_uri = local.connection_uri
    provides = {
      capabilities  = toset(["snapshot_store"])
      rust_features = toset(["dynamo"])
    }
  }
}

output "table_arns" {
  description = "ARNs of all DynamoDB tables"
  value = {
    events    = aws_dynamodb_table.events.arn
    positions = aws_dynamodb_table.positions.arn
    snapshots = aws_dynamodb_table.snapshots.arn
  }
}

output "table_names" {
  description = "Names of all DynamoDB tables"
  value = {
    events    = aws_dynamodb_table.events.name
    positions = aws_dynamodb_table.positions.name
    snapshots = aws_dynamodb_table.snapshots.name
  }
}
