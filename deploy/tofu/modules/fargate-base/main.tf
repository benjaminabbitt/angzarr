# Fargate Base Module
# Creates shared infrastructure for angzarr on AWS Fargate:
# - VPC with public/private subnets
# - ECS Cluster
# - Cloud Map namespace for service discovery
# - Application Load Balancer
# - IAM roles for task execution

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
      "managed-by"  = "opentofu"
      "environment" = var.environment
      "project"     = var.name
    },
    var.tags
  )

  vpc_id             = var.create_vpc ? aws_vpc.main[0].id : var.existing_vpc_id
  private_subnet_ids = var.create_vpc ? aws_subnet.private[*].id : var.existing_private_subnet_ids
  public_subnet_ids  = var.create_vpc ? aws_subnet.public[*].id : var.existing_public_subnet_ids
}

#------------------------------------------------------------------------------
# VPC
#------------------------------------------------------------------------------
resource "aws_vpc" "main" {
  count = var.create_vpc ? 1 : 0

  cidr_block           = var.vpc_cidr
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = merge(local.tags, { Name = "${var.name}-${var.environment}" })
}

resource "aws_internet_gateway" "main" {
  count = var.create_vpc ? 1 : 0

  vpc_id = aws_vpc.main[0].id
  tags   = merge(local.tags, { Name = "${var.name}-${var.environment}-igw" })
}

resource "aws_subnet" "public" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  vpc_id                  = aws_vpc.main[0].id
  cidr_block              = cidrsubnet(var.vpc_cidr, 4, count.index)
  availability_zone       = var.availability_zones[count.index]
  map_public_ip_on_launch = true

  tags = merge(local.tags, {
    Name = "${var.name}-${var.environment}-public-${count.index + 1}"
    Type = "public"
  })
}

resource "aws_subnet" "private" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  vpc_id            = aws_vpc.main[0].id
  cidr_block        = cidrsubnet(var.vpc_cidr, 4, count.index + length(var.availability_zones))
  availability_zone = var.availability_zones[count.index]

  tags = merge(local.tags, {
    Name = "${var.name}-${var.environment}-private-${count.index + 1}"
    Type = "private"
  })
}

resource "aws_eip" "nat" {
  count = var.create_vpc ? 1 : 0

  domain = "vpc"
  tags   = merge(local.tags, { Name = "${var.name}-${var.environment}-nat-eip" })
}

resource "aws_nat_gateway" "main" {
  count = var.create_vpc ? 1 : 0

  allocation_id = aws_eip.nat[0].id
  subnet_id     = aws_subnet.public[0].id

  tags = merge(local.tags, { Name = "${var.name}-${var.environment}-nat" })

  depends_on = [aws_internet_gateway.main]
}

resource "aws_route_table" "public" {
  count = var.create_vpc ? 1 : 0

  vpc_id = aws_vpc.main[0].id
  tags   = merge(local.tags, { Name = "${var.name}-${var.environment}-public-rt" })
}

resource "aws_route" "public_internet" {
  count = var.create_vpc ? 1 : 0

  route_table_id         = aws_route_table.public[0].id
  destination_cidr_block = "0.0.0.0/0"
  gateway_id             = aws_internet_gateway.main[0].id
}

resource "aws_route_table_association" "public" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public[0].id
}

resource "aws_route_table" "private" {
  count = var.create_vpc ? 1 : 0

  vpc_id = aws_vpc.main[0].id
  tags   = merge(local.tags, { Name = "${var.name}-${var.environment}-private-rt" })
}

resource "aws_route" "private_nat" {
  count = var.create_vpc ? 1 : 0

  route_table_id         = aws_route_table.private[0].id
  destination_cidr_block = "0.0.0.0/0"
  nat_gateway_id         = aws_nat_gateway.main[0].id
}

resource "aws_route_table_association" "private" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  subnet_id      = aws_subnet.private[count.index].id
  route_table_id = aws_route_table.private[0].id
}

#------------------------------------------------------------------------------
# ECS Cluster
#------------------------------------------------------------------------------
resource "aws_ecs_cluster" "main" {
  name = "${var.name}-${var.environment}"

  setting {
    name  = "containerInsights"
    value = "enabled"
  }

  tags = local.tags
}

resource "aws_ecs_cluster_capacity_providers" "main" {
  cluster_name = aws_ecs_cluster.main.name

  capacity_providers = ["FARGATE", "FARGATE_SPOT"]

  default_capacity_provider_strategy {
    base              = 1
    weight            = 100
    capacity_provider = "FARGATE"
  }
}

#------------------------------------------------------------------------------
# Cloud Map Namespace (Service Discovery)
#------------------------------------------------------------------------------
resource "aws_service_discovery_private_dns_namespace" "main" {
  count = var.create_service_discovery ? 1 : 0

  name        = "${var.name}.${var.environment}.local"
  description = "Service discovery namespace for ${var.name} ${var.environment}"
  vpc         = local.vpc_id

  tags = local.tags
}

#------------------------------------------------------------------------------
# Application Load Balancer
#------------------------------------------------------------------------------
resource "aws_security_group" "alb" {
  count = var.create_alb ? 1 : 0

  name        = "${var.name}-${var.environment}-alb"
  description = "Security group for ${var.name} ALB"
  vpc_id      = local.vpc_id

  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.tags, { Name = "${var.name}-${var.environment}-alb-sg" })
}

resource "aws_lb" "main" {
  count = var.create_alb ? 1 : 0

  name               = "${var.name}-${var.environment}"
  internal           = var.alb_internal
  load_balancer_type = "application"
  security_groups    = [aws_security_group.alb[0].id]
  subnets            = var.alb_internal ? local.private_subnet_ids : local.public_subnet_ids

  tags = local.tags
}

resource "aws_lb_listener" "http" {
  count = var.create_alb ? 1 : 0

  load_balancer_arn = aws_lb.main[0].arn
  port              = 80
  protocol          = "HTTP"

  default_action {
    type = var.alb_certificate_arn != null ? "redirect" : "fixed-response"

    dynamic "redirect" {
      for_each = var.alb_certificate_arn != null ? [1] : []
      content {
        port        = "443"
        protocol    = "HTTPS"
        status_code = "HTTP_301"
      }
    }

    dynamic "fixed_response" {
      for_each = var.alb_certificate_arn == null ? [1] : []
      content {
        content_type = "text/plain"
        message_body = "Not Found"
        status_code  = "404"
      }
    }
  }

  tags = local.tags
}

resource "aws_lb_listener" "https" {
  count = var.create_alb && var.alb_certificate_arn != null ? 1 : 0

  load_balancer_arn = aws_lb.main[0].arn
  port              = 443
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-TLS13-1-2-2021-06"
  certificate_arn   = var.alb_certificate_arn

  default_action {
    type = "fixed-response"
    fixed_response {
      content_type = "text/plain"
      message_body = "Not Found"
      status_code  = "404"
    }
  }

  tags = local.tags
}

#------------------------------------------------------------------------------
# IAM Roles
#------------------------------------------------------------------------------
# Task Execution Role (for ECS to pull images, write logs)
resource "aws_iam_role" "task_execution" {
  name = "${var.name}-${var.environment}-task-execution"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "ecs-tasks.amazonaws.com"
      }
    }]
  })

  tags = local.tags
}

resource "aws_iam_role_policy_attachment" "task_execution" {
  role       = aws_iam_role.task_execution.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"
}

# Additional policy for Secrets Manager access
resource "aws_iam_role_policy" "task_execution_secrets" {
  name = "secrets-access"
  role = aws_iam_role.task_execution.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "secretsmanager:GetSecretValue",
        "ssm:GetParameters"
      ]
      Resource = ["*"]
    }]
  })
}

# Default Task Role (for application to access AWS services)
resource "aws_iam_role" "task" {
  name = "${var.name}-${var.environment}-task"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "ecs-tasks.amazonaws.com"
      }
    }]
  })

  tags = local.tags
}

#------------------------------------------------------------------------------
# Default Security Group for Tasks
#------------------------------------------------------------------------------
resource "aws_security_group" "tasks" {
  name        = "${var.name}-${var.environment}-tasks"
  description = "Default security group for ${var.name} Fargate tasks"
  vpc_id      = local.vpc_id

  # Allow all outbound
  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Allow inbound from ALB
  dynamic "ingress" {
    for_each = var.create_alb ? [1] : []
    content {
      from_port       = 0
      to_port         = 65535
      protocol        = "tcp"
      security_groups = [aws_security_group.alb[0].id]
    }
  }

  # Allow inbound from same security group (service-to-service)
  ingress {
    from_port = 0
    to_port   = 65535
    protocol  = "tcp"
    self      = true
  }

  tags = merge(local.tags, { Name = "${var.name}-${var.environment}-tasks-sg" })
}

#------------------------------------------------------------------------------
# Data Sources
#------------------------------------------------------------------------------
data "aws_region" "current" {}
