# Fargate Domain Module - Outputs

output "aggregate_url" {
  description = "Service discovery hostname for aggregate (if enabled)"
  value       = var.service_discovery_namespace_id != null ? "${var.domain}-aggregate" : null
}

output "saga_urls" {
  description = "Service names for sagas"
  value = {
    for name, _ in var.sagas : name => "saga-${var.domain}-${name}"
  }
}

output "projector_urls" {
  description = "Service names for projectors"
  value = {
    for name, _ in var.projectors : name => "projector-${var.domain}-${name}"
  }
}

output "domain" {
  description = "Domain name"
  value       = var.domain
}

output "security_group_id" {
  description = "Security group ID for the domain"
  value       = aws_security_group.domain.id
}

output "service_arns" {
  description = "ECS service ARNs"
  value = {
    aggregate = aws_ecs_service.aggregate.id
    sagas = {
      for name, service in aws_ecs_service.saga : name => service.id
    }
    projectors = {
      for name, service in aws_ecs_service.projector : name => service.id
    }
  }
}

output "task_definition_arns" {
  description = "ECS task definition ARNs"
  value = {
    aggregate = aws_ecs_task_definition.aggregate.arn
    sagas = {
      for name, td in aws_ecs_task_definition.saga : name => td.arn
    }
    projectors = {
      for name, td in aws_ecs_task_definition.projector : name => td.arn
    }
  }
}
