# SNS/SQS Module - Outputs

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities  = toset(["event_bus", "pub_sub", "fan_out"])
    cloud         = "aws"
    rust_features = toset(["sns-sqs"])
    ha_mode       = "multi-az"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null  # Works with any compute
    vpc             = false # No VPC required
    capabilities    = null
    secrets_backend = null
  }
}

output "bus" {
  description = "Bus configuration for stack module"
  value = {
    type           = "sns-sqs"
    connection_uri = "sns-sqs://${var.name_prefix}"
    provides = {
      capabilities  = toset(["event_bus", "pub_sub", "fan_out"])
      rust_features = toset(["sns-sqs"])
    }
  }
}

output "connection_uri" {
  description = "Connection URI for coordinators"
  value       = "sns-sqs://${var.name_prefix}"
}

output "topic_arns" {
  description = "Map of domain name to SNS topic ARN"
  value       = { for k, v in aws_sns_topic.domain : k => v.arn }
}

output "queue_urls" {
  description = "Map of subscriber-domain to SQS queue URL"
  value       = { for k, v in aws_sqs_queue.subscriber : k => v.url }
}

output "queue_arns" {
  description = "Map of subscriber-domain to SQS queue ARN"
  value       = { for k, v in aws_sqs_queue.subscriber : k => v.arn }
}

output "dlq_arns" {
  description = "Map of subscriber-domain to DLQ ARN"
  value       = { for k, v in aws_sqs_queue.dlq : k => v.arn }
}
