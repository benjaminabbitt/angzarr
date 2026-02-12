# Fargate ECR Module
# Creates ECR repositories for angzarr container images

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

locals {
  tags = merge(
    {
      "managed-by" = "opentofu"
    },
    var.tags
  )

  # Standard angzarr repositories
  repositories = [
    "aggregate",
    "saga",
    "projector",
    "process-manager",
    "stream",
    "topology",
    "grpc-gateway",
    "upcaster"
  ]
}

resource "aws_ecr_repository" "repos" {
  for_each = toset(local.repositories)

  name                 = "${var.name_prefix}-${each.key}"
  image_tag_mutability = var.image_tag_mutability

  image_scanning_configuration {
    scan_on_push = var.scan_on_push
  }

  encryption_configuration {
    encryption_type = var.encryption_type
    kms_key         = var.encryption_type == "KMS" ? var.kms_key_arn : null
  }

  tags = merge(local.tags, { "angzarr-component" = each.key })
}

# Lifecycle policy to clean up old images
resource "aws_ecr_lifecycle_policy" "cleanup" {
  for_each = aws_ecr_repository.repos

  repository = each.value.name

  policy = jsonencode({
    rules = [
      {
        rulePriority = 1
        description  = "Keep last 10 tagged images"
        selection = {
          tagStatus   = "tagged"
          tagPrefixList = ["v", "release"]
          countType   = "imageCountMoreThan"
          countNumber = 10
        }
        action = {
          type = "expire"
        }
      },
      {
        rulePriority = 2
        description  = "Expire untagged images older than ${var.lifecycle_policy_days} days"
        selection = {
          tagStatus   = "untagged"
          countType   = "sinceImagePushed"
          countUnit   = "days"
          countNumber = var.lifecycle_policy_days
        }
        action = {
          type = "expire"
        }
      }
    ]
  })
}

# Data source for current account
data "aws_caller_identity" "current" {}
data "aws_region" "current" {}
