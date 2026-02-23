# Fargate PM Module
# Deploys a process manager as an ECS Fargate service
# Uses multiple containers per task for sidecar pattern (coordinator + business logic)

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

locals {
  # Common coordinator environment variables
  coordinator_env = [
    { name = "ANGZARR_PM_NAME", value = var.name },
    { name = "ANGZARR_EVENT_STORE", value = var.storage.event_store.connection_uri },
    { name = "ANGZARR_POSITION_STORE", value = var.storage.position_store.connection_uri },
    { name = "ANGZARR_SNAPSHOT_STORE", value = var.storage.snapshot_store != null ? var.storage.snapshot_store.connection_uri : "" },
    { name = "ANGZARR_BUS_URI", value = var.bus.connection_uri },
    { name = "ANGZARR_BUS_TYPE", value = var.bus.type },
    { name = "ANGZARR_SUBSCRIPTIONS", value = join(";", var.subscriptions) },
    { name = "ANGZARR_TARGETS", value = join(";", var.targets) },
  ]

  common_tags = merge(var.labels, {
    "angzarr-component" = "pm"
    "angzarr-pm-name"   = var.name
  })
}

#------------------------------------------------------------------------------
# Security Group
#------------------------------------------------------------------------------

resource "aws_security_group" "pm" {
  name        = "pm-${var.name}"
  description = "Security group for process manager ${var.name}"
  vpc_id      = var.vpc_id

  ingress {
    from_port   = 1310
    to_port     = 1310
    protocol    = "tcp"
    cidr_blocks = var.allowed_cidr_blocks
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.common_tags, {
    Name = "pm-${var.name}-sg"
  })
}

#------------------------------------------------------------------------------
# Process Manager
#------------------------------------------------------------------------------

resource "aws_ecs_task_definition" "pm" {
  family                   = "pm-${var.name}"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.resources.cpu
  memory                   = var.resources.memory
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = var.task_role_arn

  container_definitions = jsonencode([
    {
      name      = "coordinator"
      image     = var.coordinator_images.pm
      essential = true

      portMappings = [{
        containerPort = 1310
        protocol      = "tcp"
      }]

      environment = local.coordinator_env

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = var.log_group
          "awslogs-region"        = var.region
          "awslogs-stream-prefix" = "pm-${var.name}-coordinator"
        }
      }

      healthCheck = {
        command     = ["CMD-SHELL", "grpc_health_probe -addr=:1310 || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "logic"
      image     = var.image
      essential = true

      environment = [for k, v in var.env : { name = k, value = v }]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = var.log_group
          "awslogs-region"        = var.region
          "awslogs-stream-prefix" = "pm-${var.name}-logic"
        }
      }
    }
  ])

  tags = local.common_tags
}

resource "aws_ecs_service" "pm" {
  name            = "pm-${var.name}"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.pm.arn
  desired_count   = var.scaling.desired_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = [aws_security_group.pm.id]
    assign_public_ip = var.assign_public_ip
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.pm[0].arn
    }
  }

  tags = local.common_tags
}

resource "aws_service_discovery_service" "pm" {
  count = var.service_discovery_namespace_id != null ? 1 : 0

  name = "pm-${var.name}"

  dns_config {
    namespace_id = var.service_discovery_namespace_id

    dns_records {
      ttl  = 10
      type = "A"
    }
  }

  health_check_custom_config {}
}
