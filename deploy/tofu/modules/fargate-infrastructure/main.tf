# Fargate Infrastructure Module
# Deploys shared infrastructure services:
# - Stream: Event streaming service
# - Topology: Topology visualization service
#
# AWS Fargate equivalent of the GCP Cloud Run infrastructure module

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
      "component"  = "infrastructure"
    },
    var.tags
  )

  # Convert K8s-style resources to Fargate values
  fargate_cpu_map = {
    "0.25" = 256
    "0.5"  = 512
    "1"    = 1024
    "2"    = 2048
    "4"    = 4096
  }

  # Coordinator env as list for container definitions
  coordinator_env_list = [
    for k, v in var.coordinator_env : { name = k, value = v }
  ]

  coordinator_secrets_list = [
    for k, v in var.coordinator_secrets : {
      name      = k
      valueFrom = v.key != null ? "${v.secret_arn}:${v.key}::" : v.secret_arn
    }
  ]
}

#------------------------------------------------------------------------------
# CloudWatch Log Groups
#------------------------------------------------------------------------------
resource "aws_cloudwatch_log_group" "stream" {
  count = var.stream.enabled ? 1 : 0

  name              = "/ecs/angzarr-stream"
  retention_in_days = 30
  tags              = merge(local.tags, { "angzarr-component" = "stream" })
}

resource "aws_cloudwatch_log_group" "topology" {
  count = var.topology.enabled ? 1 : 0

  name              = "/ecs/angzarr-topology"
  retention_in_days = 30
  tags              = merge(local.tags, { "angzarr-component" = "topology" })
}

#------------------------------------------------------------------------------
# Security Group (if none provided)
#------------------------------------------------------------------------------
resource "aws_security_group" "infrastructure" {
  count = length(var.security_group_ids) == 0 ? 1 : 0

  name        = "angzarr-infrastructure"
  description = "Security group for angzarr infrastructure services"
  vpc_id      = var.vpc_id

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Stream port (gRPC)
  ingress {
    from_port   = 1340
    to_port     = 1340
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # Topology port (REST)
  ingress {
    from_port   = 9099
    to_port     = 9099
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = local.tags
}

locals {
  security_group_ids = length(var.security_group_ids) > 0 ? var.security_group_ids : [aws_security_group.infrastructure[0].id]
}

#------------------------------------------------------------------------------
# Stream Service
#------------------------------------------------------------------------------
resource "aws_ecs_task_definition" "stream" {
  count = var.stream.enabled ? 1 : 0

  family                   = "angzarr-stream"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = lookup(local.fargate_cpu_map, var.stream.resources.cpu, 1024)
  memory                   = 512
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = var.task_role_arn

  container_definitions = jsonencode([
    {
      name      = "stream"
      image     = var.stream.image
      essential = true
      portMappings = [{
        containerPort = 1340
        protocol      = "tcp"
      }]
      environment = concat(
        local.coordinator_env_list,
        [for k, v in var.stream.env : { name = k, value = v }],
        [
          { name = "RUST_LOG", value = var.log_level },
          { name = "PORT", value = "1340" }
        ]
      )
      secrets = local.coordinator_secrets_list
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.stream[0].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "stream"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:1340 || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    }
  ])

  tags = merge(local.tags, { "angzarr-component" = "stream" })
}

resource "aws_ecs_service" "stream" {
  count = var.stream.enabled ? 1 : 0

  name            = "angzarr-stream"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.stream[0].arn
  desired_count   = var.stream.min_instances > 0 ? var.stream.min_instances : 1
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = local.security_group_ids
    assign_public_ip = false
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.stream[0].arn
    }
  }

  tags = merge(local.tags, { "angzarr-component" = "stream" })
}

resource "aws_service_discovery_service" "stream" {
  count = var.stream.enabled && var.service_discovery_namespace_id != null ? 1 : 0

  name = "angzarr-stream"

  dns_config {
    namespace_id = var.service_discovery_namespace_id
    dns_records {
      ttl  = 10
      type = "A"
    }
    routing_policy = "MULTIVALUE"
  }

  health_check_custom_config {
    failure_threshold = 1
  }

  tags = merge(local.tags, { "angzarr-component" = "stream" })
}

#------------------------------------------------------------------------------
# Topology Service
#------------------------------------------------------------------------------
resource "aws_ecs_task_definition" "topology" {
  count = var.topology.enabled ? 1 : 0

  family                   = "angzarr-topology"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = lookup(local.fargate_cpu_map, var.topology.resources.cpu, 512)
  memory                   = 256
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = var.task_role_arn

  container_definitions = jsonencode([
    {
      name      = "topology"
      image     = var.topology.image
      essential = true
      portMappings = [{
        containerPort = 9099
        protocol      = "tcp"
      }]
      environment = concat(
        local.coordinator_env_list,
        [for k, v in var.topology.env : { name = k, value = v }],
        [
          { name = "RUST_LOG", value = var.log_level },
          { name = "PORT", value = "9099" }
        ]
      )
      secrets = local.coordinator_secrets_list
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.topology[0].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "topology"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "wget -q --spider http://localhost:9099/health || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    }
  ])

  tags = merge(local.tags, { "angzarr-component" = "topology" })
}

resource "aws_ecs_service" "topology" {
  count = var.topology.enabled ? 1 : 0

  name            = "angzarr-topology"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.topology[0].arn
  desired_count   = var.topology.min_instances > 0 ? var.topology.min_instances : 1
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = local.security_group_ids
    assign_public_ip = false
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.topology[0].arn
    }
  }

  dynamic "load_balancer" {
    for_each = var.lb_arn != null ? [1] : []
    content {
      target_group_arn = aws_lb_target_group.topology[0].arn
      container_name   = "topology"
      container_port   = 9099
    }
  }

  tags = merge(local.tags, { "angzarr-component" = "topology" })
}

resource "aws_service_discovery_service" "topology" {
  count = var.topology.enabled && var.service_discovery_namespace_id != null ? 1 : 0

  name = "angzarr-topology"

  dns_config {
    namespace_id = var.service_discovery_namespace_id
    dns_records {
      ttl  = 10
      type = "A"
    }
    routing_policy = "MULTIVALUE"
  }

  health_check_custom_config {
    failure_threshold = 1
  }

  tags = merge(local.tags, { "angzarr-component" = "topology" })
}

resource "aws_lb_target_group" "topology" {
  count = var.topology.enabled && var.lb_arn != null ? 1 : 0

  name        = "angzarr-topology"
  port        = 9099
  protocol    = "HTTP"
  vpc_id      = var.vpc_id
  target_type = "ip"

  health_check {
    enabled             = true
    path                = "/health"
    protocol            = "HTTP"
    healthy_threshold   = 3
    unhealthy_threshold = 3
    timeout             = 5
    interval            = 30
  }

  tags = merge(local.tags, { "angzarr-component" = "topology" })
}

#------------------------------------------------------------------------------
# Data Sources
#------------------------------------------------------------------------------
data "aws_region" "current" {}
