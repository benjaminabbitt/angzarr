# Fargate Base Module - Outputs

# VPC
output "vpc_id" {
  description = "VPC ID"
  value       = local.vpc_id
}

output "private_subnet_ids" {
  description = "Private subnet IDs"
  value       = local.private_subnet_ids
}

output "public_subnet_ids" {
  description = "Public subnet IDs"
  value       = local.public_subnet_ids
}

# ECS Cluster
output "cluster_arn" {
  description = "ECS cluster ARN"
  value       = aws_ecs_cluster.main.arn
}

output "cluster_name" {
  description = "ECS cluster name"
  value       = aws_ecs_cluster.main.name
}

# Service Discovery
output "service_discovery_namespace_id" {
  description = "Cloud Map namespace ID"
  value       = var.create_service_discovery ? aws_service_discovery_private_dns_namespace.main[0].id : null
}

output "service_discovery_namespace_name" {
  description = "Cloud Map namespace name"
  value       = var.create_service_discovery ? aws_service_discovery_private_dns_namespace.main[0].name : null
}

# Load Balancer
output "lb_arn" {
  description = "ALB ARN"
  value       = var.create_alb ? aws_lb.main[0].arn : null
}

output "lb_dns_name" {
  description = "ALB DNS name"
  value       = var.create_alb ? aws_lb.main[0].dns_name : null
}

output "lb_listener_http_arn" {
  description = "ALB HTTP listener ARN"
  value       = var.create_alb ? aws_lb_listener.http[0].arn : null
}

output "lb_listener_https_arn" {
  description = "ALB HTTPS listener ARN"
  value       = var.create_alb && var.alb_certificate_arn != null ? aws_lb_listener.https[0].arn : null
}

output "lb_security_group_id" {
  description = "ALB security group ID"
  value       = var.create_alb ? aws_security_group.alb[0].id : null
}

# IAM
output "execution_role_arn" {
  description = "Task execution role ARN"
  value       = aws_iam_role.task_execution.arn
}

output "task_role_arn" {
  description = "Default task role ARN"
  value       = aws_iam_role.task.arn
}

# Security Groups
output "tasks_security_group_id" {
  description = "Default security group ID for tasks"
  value       = aws_security_group.tasks.id
}
