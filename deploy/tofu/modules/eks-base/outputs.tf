# EKS Base Module - Outputs

output "provides" {
  description = "Capabilities provided by this module"
  value = {
    capabilities    = toset(["compute"])
    compute_type    = "eks"
    cloud           = "aws"
    region          = data.aws_region.current.name
    ha_mode         = "multi-az"
    rust_features   = []
    secrets_backend = "k8s"
  }
}

output "requirements" {
  description = "Requirements for this module"
  value = {
    compute_types   = null
    vpc             = false # Creates its own or uses provided
    capabilities    = null
    secrets_backend = null
  }
}

output "cluster_name" {
  description = "EKS cluster name"
  value       = aws_eks_cluster.angzarr.name
}

output "cluster_endpoint" {
  description = "EKS cluster endpoint"
  value       = aws_eks_cluster.angzarr.endpoint
}

output "cluster_certificate_authority_data" {
  description = "Base64 encoded certificate data for cluster"
  value       = aws_eks_cluster.angzarr.certificate_authority[0].data
}

output "cluster_arn" {
  description = "EKS cluster ARN"
  value       = aws_eks_cluster.angzarr.arn
}

output "cluster_version" {
  description = "Kubernetes version"
  value       = aws_eks_cluster.angzarr.version
}

output "oidc_provider_arn" {
  description = "OIDC provider ARN for IRSA"
  value       = aws_iam_openid_connect_provider.eks.arn
}

output "oidc_provider_url" {
  description = "OIDC provider URL"
  value       = aws_eks_cluster.angzarr.identity[0].oidc[0].issuer
}

output "vpc_id" {
  description = "VPC ID"
  value       = local.vpc_id
}

output "subnet_ids" {
  description = "Private subnet IDs for workloads"
  value       = local.subnet_ids
}

output "node_role_arn" {
  description = "IAM role ARN for node groups"
  value       = aws_iam_role.node.arn
}

output "cluster_security_group_id" {
  description = "Cluster security group ID"
  value       = aws_eks_cluster.angzarr.vpc_config[0].cluster_security_group_id
}
