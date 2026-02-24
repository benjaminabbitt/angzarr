# Fargate Domain Module
# Deploys domain components (aggregate, sagas, projectors) as ECS Fargate services
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
    { name = "ANGZARR_DOMAIN", value = var.domain },
    { name = "ANGZARR_EVENT_STORE", value = var.storage.event_store.connection_uri },
    { name = "ANGZARR_POSITION_STORE", value = var.storage.position_store.connection_uri },
    { name = "ANGZARR_SNAPSHOT_STORE", value = var.storage.snapshot_store != null ? var.storage.snapshot_store.connection_uri : "" },
    { name = "ANGZARR_BUS_URI", value = var.bus.connection_uri },
    { name = "ANGZARR_BUS_TYPE", value = var.bus.type },
  ]

  common_tags = merge(var.labels, {
    "angzarr-domain" = var.domain
  })

  # Optional gRPC Gateway container for REST API exposure
  grpc_gateway_container = var.grpc_gateway.enabled ? [{
    name      = "grpc-gateway"
    image     = var.coordinator_images.grpc_gateway
    essential = false

    portMappings = [{
      containerPort = var.grpc_gateway.port
      protocol      = "tcp"
    }]

    environment = [
      { name = "GRPC_TARGET", value = "localhost:1310" }
    ]

    logConfiguration = {
      logDriver = "awslogs"
      options = {
        "awslogs-group"         = var.log_group
        "awslogs-region"        = var.region
        "awslogs-stream-prefix" = "${var.domain}-grpc-gateway"
      }
    }

    healthCheck = {
      command     = ["CMD-SHELL", "wget -q --spider http://localhost:${var.grpc_gateway.port}/health || exit 1"]
      interval    = 30
      timeout     = 5
      retries     = 3
      startPeriod = 10
    }
  }] : []
}

#------------------------------------------------------------------------------
# Security Group
#------------------------------------------------------------------------------

resource "aws_security_group" "domain" {
  name        = "${var.domain}-domain"
  description = "Security group for ${var.domain} domain services"
  vpc_id      = var.vpc_id

  ingress {
    from_port   = 1310
    to_port     = 1310
    protocol    = "tcp"
    cidr_blocks = var.allowed_cidr_blocks
  }

  dynamic "ingress" {
    for_each = var.grpc_gateway.enabled ? [1] : []
    content {
      from_port   = var.grpc_gateway.port
      to_port     = var.grpc_gateway.port
      protocol    = "tcp"
      cidr_blocks = var.allowed_cidr_blocks
    }
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = merge(local.common_tags, {
    Name = "${var.domain}-domain-sg"
  })
}

#------------------------------------------------------------------------------
# Aggregate
#------------------------------------------------------------------------------

resource "aws_ecs_task_definition" "aggregate" {
  family                   = "${var.domain}-aggregate"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.resources.aggregate.cpu
  memory                   = var.resources.aggregate.memory
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = var.task_role_arn

  container_definitions = jsonencode(concat([
    {
      name      = "coordinator"
      image     = var.coordinator_images.aggregate
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
          "awslogs-stream-prefix" = "${var.domain}-aggregate-coordinator"
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
      image     = var.aggregate.image
      essential = true

      environment = [for k, v in var.aggregate.env : { name = k, value = v }]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = var.log_group
          "awslogs-region"        = var.region
          "awslogs-stream-prefix" = "${var.domain}-aggregate-logic"
        }
      }
    }
  ], local.grpc_gateway_container))

  tags = merge(local.common_tags, {
    "angzarr-component" = "aggregate"
  })
}

resource "aws_ecs_service" "aggregate" {
  name            = "${var.domain}-aggregate"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.aggregate.arn
  desired_count   = var.scaling.aggregate.desired_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = [aws_security_group.domain.id]
    assign_public_ip = var.assign_public_ip
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.aggregate[0].arn
    }
  }

  tags = merge(local.common_tags, {
    "angzarr-component" = "aggregate"
  })
}

resource "aws_service_discovery_service" "aggregate" {
  count = var.service_discovery_namespace_id != null ? 1 : 0

  name = "${var.domain}-aggregate"

  dns_config {
    namespace_id = var.service_discovery_namespace_id

    dns_records {
      ttl  = 10
      type = "A"
    }
  }

  health_check_custom_config {}
}

#------------------------------------------------------------------------------
# Sagas
#------------------------------------------------------------------------------

resource "aws_ecs_task_definition" "saga" {
  for_each = var.sagas

  family                   = "saga-${var.domain}-${each.key}"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.resources.saga.cpu
  memory                   = var.resources.saga.memory
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = var.task_role_arn

  container_definitions = jsonencode([
    {
      name      = "coordinator"
      image     = var.coordinator_images.saga
      essential = true

      portMappings = [{
        containerPort = 1310
        protocol      = "tcp"
      }]

      environment = concat(local.coordinator_env, [
        { name = "ANGZARR_TARGET_DOMAIN", value = each.value.target_domain }
      ])

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = var.log_group
          "awslogs-region"        = var.region
          "awslogs-stream-prefix" = "saga-${var.domain}-${each.key}-coordinator"
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
      image     = each.value.image
      essential = true

      environment = [for k, v in each.value.env : { name = k, value = v }]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = var.log_group
          "awslogs-region"        = var.region
          "awslogs-stream-prefix" = "saga-${var.domain}-${each.key}-logic"
        }
      }
    }
  ])

  tags = merge(local.common_tags, {
    "angzarr-component"     = "saga"
    "angzarr-saga-name"     = each.key
    "angzarr-target-domain" = each.value.target_domain
  })
}

resource "aws_ecs_service" "saga" {
  for_each = var.sagas

  name            = "saga-${var.domain}-${each.key}"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.saga[each.key].arn
  desired_count   = var.scaling.saga.desired_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = [aws_security_group.domain.id]
    assign_public_ip = var.assign_public_ip
  }

  tags = merge(local.common_tags, {
    "angzarr-component" = "saga"
    "angzarr-saga-name" = each.key
  })
}

#------------------------------------------------------------------------------
# Projectors
#------------------------------------------------------------------------------

resource "aws_ecs_task_definition" "projector" {
  for_each = var.projectors

  family                   = "projector-${var.domain}-${each.key}"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = var.resources.projector.cpu
  memory                   = var.resources.projector.memory
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = var.task_role_arn

  container_definitions = jsonencode([
    {
      name      = "coordinator"
      image     = var.coordinator_images.projector
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
          "awslogs-stream-prefix" = "projector-${var.domain}-${each.key}-coordinator"
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
      image     = each.value.image
      essential = true

      environment = [for k, v in each.value.env : { name = k, value = v }]

      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = var.log_group
          "awslogs-region"        = var.region
          "awslogs-stream-prefix" = "projector-${var.domain}-${each.key}-logic"
        }
      }
    }
  ])

  tags = merge(local.common_tags, {
    "angzarr-component"      = "projector"
    "angzarr-projector-name" = each.key
  })
}

resource "aws_ecs_service" "projector" {
  for_each = var.projectors

  name            = "projector-${var.domain}-${each.key}"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.projector[each.key].arn
  desired_count   = var.scaling.projector.desired_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = [aws_security_group.domain.id]
    assign_public_ip = var.assign_public_ip
  }

  tags = merge(local.common_tags, {
    "angzarr-component"      = "projector"
    "angzarr-projector-name" = each.key
  })
}
