# Fargate PM Module - Outputs

output "pm_url" {
  description = "Service discovery hostname for PM (if enabled)"
  value       = var.service_discovery_namespace_id != null ? "pm-${var.name}" : null
}

output "name" {
  description = "Process manager name"
  value       = var.name
}

output "security_group_id" {
  description = "Security group ID for the process manager"
  value       = aws_security_group.pm.id
}

output "service_arn" {
  description = "ECS service ARN"
  value       = aws_ecs_service.pm.id
}

output "task_definition_arn" {
  description = "ECS task definition ARN"
  value       = aws_ecs_task_definition.pm.arn
}

output "subscriptions" {
  description = "Domains this PM subscribes to"
  value       = var.subscriptions
}

output "targets" {
  description = "Domains this PM targets"
  value       = var.targets
}
