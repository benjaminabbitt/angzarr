# DynamoDB Module - Main
# AWS DynamoDB tables for event store, position store, and snapshot store

terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

locals {
  tags = merge(var.tags, {
    "angzarr-component" = "storage"
    "angzarr-storage"   = "dynamo"
  })
}

#------------------------------------------------------------------------------
# Events Table
#------------------------------------------------------------------------------

resource "aws_dynamodb_table" "events" {
  name         = "${var.table_prefix}-events"
  billing_mode = var.billing_mode

  # Provisioned capacity (only used if billing_mode = PROVISIONED)
  read_capacity  = var.billing_mode == "PROVISIONED" ? var.read_capacity : null
  write_capacity = var.billing_mode == "PROVISIONED" ? var.write_capacity : null

  # Primary key: aggregate_id (partition) + sequence (sort)
  hash_key  = "aggregate_id"
  range_key = "sequence"

  attribute {
    name = "aggregate_id"
    type = "S"
  }

  attribute {
    name = "sequence"
    type = "N"
  }

  attribute {
    name = "correlation_id"
    type = "S"
  }

  # GSI for correlation_id lookups (process managers)
  global_secondary_index {
    name            = "correlation_id-index"
    hash_key        = "correlation_id"
    projection_type = "ALL"

    read_capacity  = var.billing_mode == "PROVISIONED" ? var.gsi_read_capacity : null
    write_capacity = var.billing_mode == "PROVISIONED" ? var.gsi_write_capacity : null
  }

  # Point-in-time recovery
  dynamic "point_in_time_recovery" {
    for_each = var.point_in_time_recovery ? [1] : []
    content {
      enabled = true
    }
  }

  # Server-side encryption
  dynamic "server_side_encryption" {
    for_each = var.kms_key_arn != null ? [1] : []
    content {
      enabled     = true
      kms_key_arn = var.kms_key_arn
    }
  }

  # TTL (optional)
  dynamic "ttl" {
    for_each = var.ttl_attribute != null ? [1] : []
    content {
      enabled        = true
      attribute_name = var.ttl_attribute
    }
  }

  deletion_protection_enabled = var.deletion_protection

  tags = merge(local.tags, {
    "angzarr-table" = "events"
  })
}

#------------------------------------------------------------------------------
# Positions Table
#------------------------------------------------------------------------------

resource "aws_dynamodb_table" "positions" {
  name         = "${var.table_prefix}-positions"
  billing_mode = var.billing_mode

  read_capacity  = var.billing_mode == "PROVISIONED" ? var.read_capacity : null
  write_capacity = var.billing_mode == "PROVISIONED" ? var.write_capacity : null

  # Primary key: subscriber_id (partition) + domain (sort)
  hash_key  = "subscriber_id"
  range_key = "domain"

  attribute {
    name = "subscriber_id"
    type = "S"
  }

  attribute {
    name = "domain"
    type = "S"
  }

  dynamic "point_in_time_recovery" {
    for_each = var.point_in_time_recovery ? [1] : []
    content {
      enabled = true
    }
  }

  dynamic "server_side_encryption" {
    for_each = var.kms_key_arn != null ? [1] : []
    content {
      enabled     = true
      kms_key_arn = var.kms_key_arn
    }
  }

  deletion_protection_enabled = var.deletion_protection

  tags = merge(local.tags, {
    "angzarr-table" = "positions"
  })
}

#------------------------------------------------------------------------------
# Snapshots Table
#------------------------------------------------------------------------------

resource "aws_dynamodb_table" "snapshots" {
  name         = "${var.table_prefix}-snapshots"
  billing_mode = var.billing_mode

  read_capacity  = var.billing_mode == "PROVISIONED" ? var.read_capacity : null
  write_capacity = var.billing_mode == "PROVISIONED" ? var.write_capacity : null

  # Primary key: aggregate_id (partition) + version (sort)
  hash_key  = "aggregate_id"
  range_key = "version"

  attribute {
    name = "aggregate_id"
    type = "S"
  }

  attribute {
    name = "version"
    type = "N"
  }

  dynamic "point_in_time_recovery" {
    for_each = var.point_in_time_recovery ? [1] : []
    content {
      enabled = true
    }
  }

  dynamic "server_side_encryption" {
    for_each = var.kms_key_arn != null ? [1] : []
    content {
      enabled     = true
      kms_key_arn = var.kms_key_arn
    }
  }

  dynamic "ttl" {
    for_each = var.snapshot_ttl_attribute != null ? [1] : []
    content {
      enabled        = true
      attribute_name = var.snapshot_ttl_attribute
    }
  }

  deletion_protection_enabled = var.deletion_protection

  tags = merge(local.tags, {
    "angzarr-table" = "snapshots"
  })
}
