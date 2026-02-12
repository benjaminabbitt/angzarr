# Fargate ECR Module - Outputs

output "repository_urls" {
  description = "Map of component name to ECR repository URL"
  value = {
    for name, repo in aws_ecr_repository.repos : name => repo.repository_url
  }
}

output "repository_arns" {
  description = "Map of component name to ECR repository ARN"
  value = {
    for name, repo in aws_ecr_repository.repos : name => repo.arn
  }
}

output "registry_id" {
  description = "ECR registry ID (AWS account ID)"
  value       = data.aws_caller_identity.current.account_id
}

output "registry_url" {
  description = "ECR registry URL (without repository name)"
  value       = "${data.aws_caller_identity.current.account_id}.dkr.ecr.${data.aws_region.current.name}.amazonaws.com"
}

# Convenience outputs for common images
output "images" {
  description = "Convenience map of component to full image URL (without tag)"
  value = {
    grpc_gateway          = aws_ecr_repository.repos["grpc-gateway"].repository_url
    coordinator_aggregate = aws_ecr_repository.repos["aggregate"].repository_url
    coordinator_saga      = aws_ecr_repository.repos["saga"].repository_url
    coordinator_projector = aws_ecr_repository.repos["projector"].repository_url
    coordinator_pm        = aws_ecr_repository.repos["process-manager"].repository_url
    stream                = aws_ecr_repository.repos["stream"].repository_url
    topology              = aws_ecr_repository.repos["topology"].repository_url
    upcaster              = aws_ecr_repository.repos["upcaster"].repository_url
  }
}
