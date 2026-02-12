# Fargate Domain Module - Outputs

output "component_service_name" {
  description = "Name of the main component ECS service (aggregate or process manager)"
  value = var.aggregate.enabled ? (
    length(aws_ecs_service.aggregate) > 0 ? aws_ecs_service.aggregate[0].name : null
    ) : (
    var.process_manager.enabled && length(aws_ecs_service.process_manager) > 0 ? aws_ecs_service.process_manager[0].name : null
  )
}

output "component_service_arn" {
  description = "ARN of the main component ECS service"
  value = var.aggregate.enabled ? (
    length(aws_ecs_service.aggregate) > 0 ? aws_ecs_service.aggregate[0].id : null
    ) : (
    var.process_manager.enabled && length(aws_ecs_service.process_manager) > 0 ? aws_ecs_service.process_manager[0].id : null
  )
}

output "saga_service_names" {
  description = "Map of saga name to ECS service name"
  value = {
    for name, service in aws_ecs_service.saga : name => service.name
  }
}

output "projector_service_names" {
  description = "Map of projector name to ECS service name"
  value = {
    for name, service in aws_ecs_service.projector : name => service.name
  }
}

output "task_role_arn" {
  description = "Task role ARN for this domain"
  value       = local.task_role_arn
}

output "security_group_ids" {
  description = "Security group IDs used by tasks"
  value       = local.security_group_ids
}

# Service discovery DNS names
output "service_discovery_dns" {
  description = "Cloud Map DNS names for service discovery"
  value = {
    aggregate = var.aggregate.enabled && var.service_discovery_namespace_id != null && length(aws_service_discovery_service.aggregate) > 0 ? (
      aws_service_discovery_service.aggregate[0].name
    ) : null
    process_manager = var.process_manager.enabled && var.service_discovery_namespace_id != null && length(aws_service_discovery_service.process_manager) > 0 ? (
      aws_service_discovery_service.process_manager[0].name
    ) : null
    sagas = {
      for name, svc in aws_service_discovery_service.saga : name => svc.name
    }
    projectors = {
      for name, svc in aws_service_discovery_service.projector : name => svc.name
    }
  }
}

# Load balancer target groups (only aggregates exposed via ALB)
output "target_group_arns" {
  description = "ALB target group ARNs"
  value = {
    aggregate = var.aggregate.enabled && var.lb_arn != null && length(aws_lb_target_group.aggregate) > 0 ? (
      aws_lb_target_group.aggregate[0].arn
    ) : null
  }
}

# Discovery entries for environment variable injection (mirrors GCP module)
output "discovery_entries" {
  description = "All service discovery names for environment variable aggregation"
  value = merge(
    # Main component (aggregate or PM)
    var.aggregate.enabled && var.service_discovery_namespace_id != null && length(aws_service_discovery_service.aggregate) > 0 ? {
      "ANGZARR_AGGREGATE_${upper(var.domain)}" = aws_service_discovery_service.aggregate[0].name
    } : {},
    var.process_manager.enabled && var.service_discovery_namespace_id != null && length(aws_service_discovery_service.process_manager) > 0 ? {
      "ANGZARR_PM_${upper(var.domain)}" = aws_service_discovery_service.process_manager[0].name
    } : {},
    # Sagas
    {
      for name, svc in aws_service_discovery_service.saga :
      "ANGZARR_SAGA_${upper(var.domain)}_${upper(name)}" => svc.name
    },
    # Projectors
    {
      for name, svc in aws_service_discovery_service.projector :
      "ANGZARR_PROJECTOR_${upper(var.domain)}_${upper(name)}" => svc.name
    }
  )
}

# Structured discovery for JSON format (mirrors GCP module)
output "discovery_json" {
  description = "Structured discovery data for JSON serialization"
  value = {
    domain = var.domain
    aggregate = var.aggregate.enabled && length(aws_ecs_service.aggregate) > 0 ? {
      service_name = aws_ecs_service.aggregate[0].name
      service_arn  = aws_ecs_service.aggregate[0].id
      dns_name     = var.service_discovery_namespace_id != null && length(aws_service_discovery_service.aggregate) > 0 ? aws_service_discovery_service.aggregate[0].name : null
    } : null
    process_manager = var.process_manager.enabled && length(aws_ecs_service.process_manager) > 0 ? {
      service_name   = aws_ecs_service.process_manager[0].name
      service_arn    = aws_ecs_service.process_manager[0].id
      dns_name       = var.service_discovery_namespace_id != null && length(aws_service_discovery_service.process_manager) > 0 ? aws_service_discovery_service.process_manager[0].name : null
      source_domains = var.process_manager.source_domains
    } : null
    sagas = {
      for name, svc in aws_ecs_service.saga : name => {
        service_name  = svc.name
        service_arn   = svc.id
        dns_name      = var.service_discovery_namespace_id != null ? aws_service_discovery_service.saga[name].name : null
        target_domain = var.sagas[name].target_domain
      }
    }
    projectors = {
      for name, svc in aws_ecs_service.projector : name => {
        service_name = svc.name
        service_arn  = svc.id
        dns_name     = var.service_discovery_namespace_id != null ? aws_service_discovery_service.projector[name].name : null
      }
    }
  }
}

# CloudWatch log group names
output "log_groups" {
  description = "CloudWatch log group names for all components"
  value = {
    aggregate       = var.aggregate.enabled ? aws_cloudwatch_log_group.aggregate[0].name : null
    process_manager = var.process_manager.enabled ? aws_cloudwatch_log_group.process_manager[0].name : null
    sagas           = { for name, lg in aws_cloudwatch_log_group.saga : name => lg.name }
    projectors      = { for name, lg in aws_cloudwatch_log_group.projector : name => lg.name }
  }
}
