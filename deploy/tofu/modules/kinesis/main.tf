# Kinesis Module
# Creates AWS Kinesis Data Streams for angzarr event bus
#
# Architecture:
# - Per-domain streams: angzarr-events-{domain}
# - Partition key: aggregate root ID (preserves ordering within aggregate)
# - DLQ streams: angzarr-dlq-{domain}
# - Enhanced fan-out consumers for low-latency delivery
#
# NOTE: Rust bus implementation for Kinesis is TBD.
# This module provisions infrastructure in preparation.

terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

locals {
  tags = merge(
    {
      "managed-by" = "opentofu"
      "component"  = "event-bus"
    },
    var.tags
  )

  # Build stream names
  stream_names = { for domain in var.domains : domain => "${var.stream_prefix}-events-${domain}" }
  dlq_names    = { for domain in var.domains : domain => "${var.stream_prefix}-dlq-${domain}" }
}

#------------------------------------------------------------------------------
# Event Streams (per domain)
#------------------------------------------------------------------------------
resource "aws_kinesis_stream" "events" {
  for_each = toset(var.domains)

  name             = local.stream_names[each.value]
  retention_period = var.retention_hours

  # Capacity mode: ON_DEMAND or PROVISIONED
  stream_mode_details {
    stream_mode = var.stream_mode
  }

  # Only set shard_count for PROVISIONED mode
  shard_count = var.stream_mode == "PROVISIONED" ? var.shard_count : null

  # Server-side encryption
  encryption_type = var.encryption_type
  kms_key_id      = var.encryption_type == "KMS" ? var.kms_key_id : null

  tags = merge(local.tags, {
    "angzarr-domain" = each.value
    "angzarr-type"   = "events"
  })
}

#------------------------------------------------------------------------------
# Dead Letter Streams (per domain)
#------------------------------------------------------------------------------
resource "aws_kinesis_stream" "dlq" {
  for_each = var.enable_dlq ? toset(var.domains) : toset([])

  name             = local.dlq_names[each.value]
  retention_period = var.dlq_retention_hours

  stream_mode_details {
    stream_mode = var.stream_mode
  }

  shard_count = var.stream_mode == "PROVISIONED" ? var.dlq_shard_count : null

  encryption_type = var.encryption_type
  kms_key_id      = var.encryption_type == "KMS" ? var.kms_key_id : null

  tags = merge(local.tags, {
    "angzarr-domain" = each.value
    "angzarr-type"   = "dlq"
  })
}

#------------------------------------------------------------------------------
# Enhanced Fan-Out Consumers
# Each consumer gets dedicated throughput (2 MB/s read per shard)
#------------------------------------------------------------------------------
resource "aws_kinesis_stream_consumer" "consumers" {
  for_each = var.enhanced_fanout_consumers

  name       = each.key
  stream_arn = aws_kinesis_stream.events[each.value.domain].arn
}

#------------------------------------------------------------------------------
# IAM Policy: Producer (aggregates publish events)
#------------------------------------------------------------------------------
data "aws_iam_policy_document" "producer" {
  statement {
    sid    = "KinesisProducer"
    effect = "Allow"

    actions = [
      "kinesis:PutRecord",
      "kinesis:PutRecords",
      "kinesis:DescribeStream",
      "kinesis:DescribeStreamSummary",
    ]

    resources = [for stream in aws_kinesis_stream.events : stream.arn]
  }

  # DLQ write access for producers (to send failed events)
  dynamic "statement" {
    for_each = var.enable_dlq ? [1] : []
    content {
      sid    = "KinesisDLQWrite"
      effect = "Allow"

      actions = [
        "kinesis:PutRecord",
        "kinesis:PutRecords",
      ]

      resources = [for stream in aws_kinesis_stream.dlq : stream.arn]
    }
  }

  # KMS permissions if using CMK encryption
  dynamic "statement" {
    for_each = var.encryption_type == "KMS" && var.kms_key_id != null ? [1] : []
    content {
      sid    = "KMSEncrypt"
      effect = "Allow"

      actions = [
        "kms:GenerateDataKey",
        "kms:Encrypt",
      ]

      resources = [var.kms_key_id]
    }
  }
}

resource "aws_iam_policy" "producer" {
  name        = "${var.stream_prefix}-kinesis-producer"
  description = "Allows publishing events to angzarr Kinesis streams"
  policy      = data.aws_iam_policy_document.producer.json

  tags = local.tags
}

#------------------------------------------------------------------------------
# IAM Policy: Consumer (sagas, projectors, process managers)
#------------------------------------------------------------------------------
data "aws_iam_policy_document" "consumer" {
  statement {
    sid    = "KinesisConsumer"
    effect = "Allow"

    actions = [
      "kinesis:GetRecords",
      "kinesis:GetShardIterator",
      "kinesis:DescribeStream",
      "kinesis:DescribeStreamSummary",
      "kinesis:ListShards",
      "kinesis:SubscribeToShard", # Enhanced fan-out
    ]

    resources = [for stream in aws_kinesis_stream.events : stream.arn]
  }

  # Enhanced fan-out consumer ARNs
  dynamic "statement" {
    for_each = length(var.enhanced_fanout_consumers) > 0 ? [1] : []
    content {
      sid    = "KinesisEnhancedFanout"
      effect = "Allow"

      actions = [
        "kinesis:SubscribeToShard",
      ]

      resources = [for consumer in aws_kinesis_stream_consumer.consumers : consumer.arn]
    }
  }

  # KMS permissions if using CMK encryption
  dynamic "statement" {
    for_each = var.encryption_type == "KMS" && var.kms_key_id != null ? [1] : []
    content {
      sid    = "KMSDecrypt"
      effect = "Allow"

      actions = [
        "kms:Decrypt",
      ]

      resources = [var.kms_key_id]
    }
  }
}

resource "aws_iam_policy" "consumer" {
  name        = "${var.stream_prefix}-kinesis-consumer"
  description = "Allows consuming events from angzarr Kinesis streams"
  policy      = data.aws_iam_policy_document.consumer.json

  tags = local.tags
}

#------------------------------------------------------------------------------
# IAM Policy: DLQ Consumer (for replay/debugging)
#------------------------------------------------------------------------------
data "aws_iam_policy_document" "dlq_consumer" {
  count = var.enable_dlq ? 1 : 0

  statement {
    sid    = "KinesisDLQConsumer"
    effect = "Allow"

    actions = [
      "kinesis:GetRecords",
      "kinesis:GetShardIterator",
      "kinesis:DescribeStream",
      "kinesis:DescribeStreamSummary",
      "kinesis:ListShards",
    ]

    resources = [for stream in aws_kinesis_stream.dlq : stream.arn]
  }

  dynamic "statement" {
    for_each = var.encryption_type == "KMS" && var.kms_key_id != null ? [1] : []
    content {
      sid    = "KMSDecrypt"
      effect = "Allow"

      actions = [
        "kms:Decrypt",
      ]

      resources = [var.kms_key_id]
    }
  }
}

resource "aws_iam_policy" "dlq_consumer" {
  count = var.enable_dlq ? 1 : 0

  name        = "${var.stream_prefix}-kinesis-dlq-consumer"
  description = "Allows consuming from angzarr Kinesis DLQ streams"
  policy      = data.aws_iam_policy_document.dlq_consumer[0].json

  tags = local.tags
}

#------------------------------------------------------------------------------
# CloudWatch Alarms (optional)
#------------------------------------------------------------------------------
resource "aws_cloudwatch_metric_alarm" "iterator_age" {
  for_each = var.enable_alarms ? toset(var.domains) : toset([])

  alarm_name          = "${local.stream_names[each.value]}-iterator-age"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "GetRecords.IteratorAgeMilliseconds"
  namespace           = "AWS/Kinesis"
  period              = 300
  statistic           = "Maximum"
  threshold           = var.iterator_age_threshold_ms
  alarm_description   = "Consumer lag for ${each.value} domain events"

  dimensions = {
    StreamName = local.stream_names[each.value]
  }

  alarm_actions = var.alarm_actions
  ok_actions    = var.alarm_actions

  tags = local.tags
}

resource "aws_cloudwatch_metric_alarm" "write_throughput" {
  for_each = var.enable_alarms && var.stream_mode == "PROVISIONED" ? toset(var.domains) : toset([])

  alarm_name          = "${local.stream_names[each.value]}-write-throughput"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "WriteProvisionedThroughputExceeded"
  namespace           = "AWS/Kinesis"
  period              = 60
  statistic           = "Sum"
  threshold           = 0
  alarm_description   = "Write throughput exceeded for ${each.value} domain events"

  dimensions = {
    StreamName = local.stream_names[each.value]
  }

  alarm_actions = var.alarm_actions

  tags = local.tags
}
