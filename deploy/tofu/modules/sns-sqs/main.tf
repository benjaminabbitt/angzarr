# SNS/SQS Module - Main
# AWS SNS topics + SQS queues for event bus

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
    "angzarr-component" = "event-bus"
    "angzarr-bus-type"  = "sns-sqs"
  })

  fifo_suffix = var.fifo ? ".fifo" : ""
}

#------------------------------------------------------------------------------
# SNS Topics (one per domain)
#------------------------------------------------------------------------------

resource "aws_sns_topic" "domain" {
  for_each = toset(var.domains)

  name       = "${var.name_prefix}-events-${each.key}${local.fifo_suffix}"
  fifo_topic = var.fifo

  # Content-based deduplication for FIFO
  content_based_deduplication = var.fifo

  # Encryption
  kms_master_key_id = var.kms_key_id

  tags = merge(local.tags, {
    "angzarr-domain" = each.key
  })
}

#------------------------------------------------------------------------------
# SQS Queues (one per subscriber per domain)
#------------------------------------------------------------------------------

resource "aws_sqs_queue" "subscriber" {
  for_each = {
    for pair in flatten([
      for sub_name, sub_domains in var.subscribers : [
        for domain in sub_domains : {
          key    = "${sub_name}-${domain}"
          sub    = sub_name
          domain = domain
        }
      ]
    ]) : pair.key => pair
  }

  name       = "${var.name_prefix}-${each.value.sub}-${each.value.domain}${local.fifo_suffix}"
  fifo_queue = var.fifo

  # Content-based deduplication for FIFO
  content_based_deduplication = var.fifo

  # Message settings
  message_retention_seconds  = var.message_retention_seconds
  visibility_timeout_seconds = var.visibility_timeout_seconds

  # Encryption
  kms_master_key_id = var.kms_key_id

  # Redrive policy (DLQ)
  redrive_policy = var.enable_dlq ? jsonencode({
    deadLetterTargetArn = aws_sqs_queue.dlq[each.key].arn
    maxReceiveCount     = var.max_receive_count
  }) : null

  tags = merge(local.tags, {
    "angzarr-subscriber" = each.value.sub
    "angzarr-domain"     = each.value.domain
  })
}

#------------------------------------------------------------------------------
# Dead Letter Queues
#------------------------------------------------------------------------------

resource "aws_sqs_queue" "dlq" {
  for_each = var.enable_dlq ? {
    for pair in flatten([
      for sub_name, sub_domains in var.subscribers : [
        for domain in sub_domains : {
          key    = "${sub_name}-${domain}"
          sub    = sub_name
          domain = domain
        }
      ]
    ]) : pair.key => pair
  } : {}

  name       = "${var.name_prefix}-${each.value.sub}-${each.value.domain}-dlq${local.fifo_suffix}"
  fifo_queue = var.fifo

  message_retention_seconds = 1209600 # 14 days

  kms_master_key_id = var.kms_key_id

  tags = merge(local.tags, {
    "angzarr-subscriber" = each.value.sub
    "angzarr-domain"     = each.value.domain
    "angzarr-dlq"        = "true"
  })
}

#------------------------------------------------------------------------------
# SNS Subscriptions
#------------------------------------------------------------------------------

resource "aws_sns_topic_subscription" "subscriber" {
  for_each = {
    for pair in flatten([
      for sub_name, sub_domains in var.subscribers : [
        for domain in sub_domains : {
          key    = "${sub_name}-${domain}"
          sub    = sub_name
          domain = domain
        }
      ]
    ]) : pair.key => pair
  }

  topic_arn = aws_sns_topic.domain[each.value.domain].arn
  protocol  = "sqs"
  endpoint  = aws_sqs_queue.subscriber[each.key].arn

  # Raw message delivery (no SNS envelope)
  raw_message_delivery = true
}

#------------------------------------------------------------------------------
# SQS Queue Policies (allow SNS to send messages)
#------------------------------------------------------------------------------

resource "aws_sqs_queue_policy" "subscriber" {
  for_each = aws_sqs_queue.subscriber

  queue_url = each.value.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid       = "AllowSNSPublish"
        Effect    = "Allow"
        Principal = { Service = "sns.amazonaws.com" }
        Action    = "sqs:SendMessage"
        Resource  = each.value.arn
        Condition = {
          ArnLike = {
            "aws:SourceArn" = "arn:aws:sns:*:*:${var.name_prefix}-events-*"
          }
        }
      }
    ]
  })
}

#------------------------------------------------------------------------------
# CloudWatch Alarms (optional)
#------------------------------------------------------------------------------

resource "aws_cloudwatch_metric_alarm" "dlq_messages" {
  for_each = var.enable_alarms && var.enable_dlq ? aws_sqs_queue.dlq : {}

  alarm_name          = "${each.value.name}-messages"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "ApproximateNumberOfMessagesVisible"
  namespace           = "AWS/SQS"
  period              = 300
  statistic           = "Sum"
  threshold           = 0
  alarm_description   = "Messages in DLQ ${each.value.name}"

  dimensions = {
    QueueName = each.value.name
  }

  tags = local.tags
}
