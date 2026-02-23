# Kinesis Module - Outputs

#------------------------------------------------------------------------------
# Standard Interface
#------------------------------------------------------------------------------

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities  = toset(["event_bus", "pub_sub", "partitioning", "fan_out"])
    cloud         = "aws"
    rust_features = toset(["kinesis"])
    ha_mode       = "multi-az"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null
    vpc             = false
    capabilities    = null
    secrets_backend = null
  }
}

output "bus" {
  description = "Bus configuration for stack module"
  value = {
    type           = "kinesis"
    connection_uri = "kinesis://${var.stream_prefix}"
    provides = {
      capabilities  = toset(["event_bus", "pub_sub", "partitioning", "fan_out"])
      rust_features = toset(["kinesis"])
    }
  }
}

output "connection_uri" {
  description = "Connection URI for coordinators"
  value       = "kinesis://${var.stream_prefix}"
}

#------------------------------------------------------------------------------
# Stream Information
#------------------------------------------------------------------------------
output "stream_arns" {
  description = "Map of domain names to stream ARNs"
  value       = { for domain, stream in aws_kinesis_stream.events : domain => stream.arn }
}

output "stream_names" {
  description = "Map of domain names to stream names"
  value       = { for domain, stream in aws_kinesis_stream.events : domain => stream.name }
}

output "dlq_stream_arns" {
  description = "Map of domain names to DLQ stream ARNs"
  value       = { for domain, stream in aws_kinesis_stream.dlq : domain => stream.arn }
}

output "dlq_stream_names" {
  description = "Map of domain names to DLQ stream names"
  value       = { for domain, stream in aws_kinesis_stream.dlq : domain => stream.name }
}

#------------------------------------------------------------------------------
# Enhanced Fan-Out Consumers
#------------------------------------------------------------------------------
output "consumer_arns" {
  description = "Map of consumer names to their ARNs"
  value       = { for name, consumer in aws_kinesis_stream_consumer.consumers : name => consumer.arn }
}

#------------------------------------------------------------------------------
# IAM Policies
#------------------------------------------------------------------------------
output "producer_policy_arn" {
  description = "ARN of the IAM policy for producers"
  value       = aws_iam_policy.producer.arn
}

output "consumer_policy_arn" {
  description = "ARN of the IAM policy for consumers"
  value       = aws_iam_policy.consumer.arn
}

output "dlq_consumer_policy_arn" {
  description = "ARN of the IAM policy for DLQ consumers"
  value       = var.enable_dlq ? aws_iam_policy.dlq_consumer[0].arn : null
}

#------------------------------------------------------------------------------
# Environment Variables for Coordinator
#------------------------------------------------------------------------------
output "coordinator_env" {
  description = "Environment variables for angzarr coordinator configuration"
  value = {
    BUS_TYPE       = "kinesis"
    KINESIS_PREFIX = var.stream_prefix
    # Domains are discovered via stream naming convention
    # Individual stream ARNs available via stream_arns output for IAM
  }
}

output "messaging_uri" {
  description = "Messaging URI for angzarr (kinesis://<prefix>)"
  value       = "kinesis://${var.stream_prefix}"
}

#------------------------------------------------------------------------------
# CloudWatch Alarms
#------------------------------------------------------------------------------
output "alarm_arns" {
  description = "Map of alarm names to their ARNs"
  value = merge(
    { for domain, alarm in aws_cloudwatch_metric_alarm.iterator_age : "${domain}-iterator-age" => alarm.arn },
    { for domain, alarm in aws_cloudwatch_metric_alarm.write_throughput : "${domain}-write-throughput" => alarm.arn }
  )
}
