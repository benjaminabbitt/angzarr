# Fargate Infrastructure Module - Outputs

output "stream_service_name" {
  description = "Stream ECS service name"
  value       = var.stream.enabled ? aws_ecs_service.stream[0].name : null
}

output "stream_service_arn" {
  description = "Stream ECS service ARN"
  value       = var.stream.enabled ? aws_ecs_service.stream[0].id : null
}

output "topology_service_name" {
  description = "Topology ECS service name"
  value       = var.topology.enabled ? aws_ecs_service.topology[0].name : null
}

output "topology_service_arn" {
  description = "Topology ECS service ARN"
  value       = var.topology.enabled ? aws_ecs_service.topology[0].id : null
}

output "topology_target_group_arn" {
  description = "Topology ALB target group ARN"
  value       = var.topology.enabled && var.lb_arn != null ? aws_lb_target_group.topology[0].arn : null
}

output "service_discovery_dns" {
  description = "Cloud Map DNS names"
  value = {
    stream   = var.stream.enabled && var.service_discovery_namespace_id != null ? aws_service_discovery_service.stream[0].name : null
    topology = var.topology.enabled && var.service_discovery_namespace_id != null ? aws_service_discovery_service.topology[0].name : null
  }
}

output "log_groups" {
  description = "CloudWatch log group names"
  value = {
    stream   = var.stream.enabled ? aws_cloudwatch_log_group.stream[0].name : null
    topology = var.topology.enabled ? aws_cloudwatch_log_group.topology[0].name : null
  }
}

output "security_group_ids" {
  description = "Security group IDs used by infrastructure services"
  value       = local.security_group_ids
}
