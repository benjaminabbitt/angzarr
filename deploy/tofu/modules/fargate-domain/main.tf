# Fargate Domain Module
# Deploys all components for a single domain on AWS Fargate
#
# Business config (portable): aggregate, process_manager, sagas, projectors
# Operational config (AWS native): images, scaling, networking, iam, secrets

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

check "aggregate_pm_exclusive" {
  assert {
    condition     = !(var.aggregate.enabled && var.process_manager.enabled)
    error_message = "aggregate and process_manager are mutually exclusive. A domain can be either an aggregate (command handler) or a process manager (cross-domain orchestrator), not both."
  }
}

locals {
  tags = merge(
    {
      "angzarr-domain" = var.domain
      "managed-by"     = "opentofu"
    },
    var.tags
  )

  # Coordinator env shared by all components
  base_coordinator_env = merge(
    {
      "RUST_LOG"       = var.log_level
      "TRANSPORT_TYPE" = "tcp"
      "DOMAIN"         = var.domain
    },
    var.discovery_env,
    var.coordinator_env
  )

  # Port configuration
  ports = {
    grpc_gateway = 8080
    coordinator  = 1310
    logic        = 50053
    upcaster     = 50054
  }

  # Scaling defaults
  default_scaling = {
    min_instances = 1
    max_instances = 10
    cpu           = 1024
    memory        = 512
  }

  scaling_aggregate = merge(local.default_scaling, try(var.scaling.aggregate, {}))
  scaling_pm        = merge(local.default_scaling, try(var.scaling.process_manager, {}))

  # Per-saga/projector scaling with defaults
  scaling_sagas = {
    for name, _ in var.sagas :
    name => merge(local.default_scaling, try(var.scaling.sagas[name], {}))
  }

  scaling_projectors = {
    for name, _ in var.projectors :
    name => merge(local.default_scaling, try(var.scaling.projectors[name], {}))
  }

  # Convert env map to container format
  base_coordinator_env_list = [
    for k, v in local.base_coordinator_env : { name = k, value = v }
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
resource "aws_cloudwatch_log_group" "aggregate" {
  count             = var.aggregate.enabled ? 1 : 0
  name              = "/ecs/${var.domain}-aggregate"
  retention_in_days = 30
  tags              = merge(local.tags, { "angzarr-component" = "aggregate" })
}

resource "aws_cloudwatch_log_group" "process_manager" {
  count             = var.process_manager.enabled ? 1 : 0
  name              = "/ecs/${var.domain}-pm"
  retention_in_days = 30
  tags              = merge(local.tags, { "angzarr-component" = "process-manager" })
}

resource "aws_cloudwatch_log_group" "saga" {
  for_each          = var.sagas
  name              = "/ecs/saga-${var.domain}-${each.key}"
  retention_in_days = 30
  tags = merge(local.tags, {
    "angzarr-component"     = "saga"
    "angzarr-target-domain" = each.value.target_domain
  })
}

resource "aws_cloudwatch_log_group" "projector" {
  for_each          = var.projectors
  name              = "/ecs/projector-${var.domain}-${each.key}"
  retention_in_days = 30
  tags = merge(local.tags, {
    "angzarr-component" = "projector"
    "projector-name"    = each.key
  })
}

#------------------------------------------------------------------------------
# Task Role
#------------------------------------------------------------------------------
resource "aws_iam_role" "task" {
  count = var.create_task_role ? 1 : 0
  name  = "${var.domain}-domain-task-role"
  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action    = "sts:AssumeRole"
      Effect    = "Allow"
      Principal = { Service = "ecs-tasks.amazonaws.com" }
    }]
  })
  tags = local.tags
}

locals {
  task_role_arn = var.create_task_role ? aws_iam_role.task[0].arn : var.task_role_arn
}

#------------------------------------------------------------------------------
# Security Group
#------------------------------------------------------------------------------
resource "aws_security_group" "tasks" {
  count       = length(var.security_group_ids) == 0 ? 1 : 0
  name        = "${var.domain}-domain-tasks"
  description = "Security group for ${var.domain} domain Fargate tasks"
  vpc_id      = var.vpc_id

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    from_port   = local.ports.grpc_gateway
    to_port     = local.ports.grpc_gateway
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = local.tags
}

locals {
  security_group_ids = length(var.security_group_ids) > 0 ? var.security_group_ids : [aws_security_group.tasks[0].id]
}

#------------------------------------------------------------------------------
# Aggregate Task Definition & Service
#------------------------------------------------------------------------------
resource "aws_ecs_task_definition" "aggregate" {
  count = var.aggregate.enabled ? 1 : 0

  family                   = "${var.domain}-aggregate"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = local.scaling_aggregate.cpu + 512 + 256 + (var.aggregate.upcaster.enabled ? 256 : 0)
  memory                   = local.scaling_aggregate.memory + 512 + 128 + (var.aggregate.upcaster.enabled ? 128 : 0)
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = local.task_role_arn

  container_definitions = jsonencode(concat(
    [
      {
        name         = "grpc-gateway"
        image        = var.images.grpc_gateway
        essential    = true
        portMappings = [{ containerPort = local.ports.grpc_gateway, protocol = "tcp" }]
        environment = [
          { name = "GRPC_BACKEND", value = "localhost:${local.ports.coordinator}" },
          { name = "PORT", value = tostring(local.ports.grpc_gateway) }
        ]
        logConfiguration = {
          logDriver = "awslogs"
          options = {
            "awslogs-group"         = aws_cloudwatch_log_group.aggregate[0].name
            "awslogs-region"        = data.aws_region.current.name
            "awslogs-stream-prefix" = "grpc-gateway"
          }
        }
        healthCheck = {
          command     = ["CMD-SHELL", "wget -q --spider http://localhost:${local.ports.grpc_gateway}/health || exit 1"]
          interval    = 30
          timeout     = 5
          retries     = 3
          startPeriod = 60
        }
      },
      {
        name      = "coordinator"
        image     = var.images.coordinator_aggregate
        essential = true
        environment = concat(
          local.base_coordinator_env_list,
          [
            { name = "PORT", value = tostring(local.ports.coordinator) },
            { name = "COMPONENT_TYPE", value = "aggregate" },
            { name = "ANGZARR__TARGET__ADDRESS", value = "localhost:${local.ports.logic}" },
            { name = "ANGZARR_UPCASTER_ENABLED", value = tostring(var.aggregate.upcaster.enabled) },
            { name = "ANGZARR_UPCASTER_ADDRESS", value = var.aggregate.upcaster.enabled ? "localhost:${local.ports.upcaster}" : "" }
          ]
        )
        secrets = local.coordinator_secrets_list
        logConfiguration = {
          logDriver = "awslogs"
          options = {
            "awslogs-group"         = aws_cloudwatch_log_group.aggregate[0].name
            "awslogs-region"        = data.aws_region.current.name
            "awslogs-stream-prefix" = "coordinator"
          }
        }
        healthCheck = {
          command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.coordinator} || exit 1"]
          interval    = 30
          timeout     = 5
          retries     = 3
          startPeriod = 60
        }
      },
      {
        name      = "logic"
        image     = var.images.logic
        essential = true
        environment = concat(
          [for k, v in var.aggregate.env : { name = k, value = v }],
          [
            { name = "RUST_LOG", value = var.log_level },
            { name = "PORT", value = tostring(local.ports.logic) }
          ]
        )
        logConfiguration = {
          logDriver = "awslogs"
          options = {
            "awslogs-group"         = aws_cloudwatch_log_group.aggregate[0].name
            "awslogs-region"        = data.aws_region.current.name
            "awslogs-stream-prefix" = "logic"
          }
        }
        healthCheck = {
          command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.logic} || exit 1"]
          interval    = 30
          timeout     = 5
          retries     = 3
          startPeriod = 60
        }
      }
    ],
    var.aggregate.upcaster.enabled ? [
      {
        name      = "upcaster"
        image     = var.images.upcaster
        essential = false
        environment = concat(
          [for k, v in var.aggregate.upcaster.env : { name = k, value = v }],
          [
            { name = "RUST_LOG", value = var.log_level },
            { name = "PORT", value = tostring(local.ports.upcaster) },
            { name = "DOMAIN", value = var.domain }
          ]
        )
        logConfiguration = {
          logDriver = "awslogs"
          options = {
            "awslogs-group"         = aws_cloudwatch_log_group.aggregate[0].name
            "awslogs-region"        = data.aws_region.current.name
            "awslogs-stream-prefix" = "upcaster"
          }
        }
        healthCheck = {
          command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.upcaster} || exit 1"]
          interval    = 30
          timeout     = 5
          retries     = 3
          startPeriod = 60
        }
      }
    ] : []
  ))

  tags = merge(local.tags, { "angzarr-component" = "aggregate" })
}

resource "aws_ecs_service" "aggregate" {
  count = var.aggregate.enabled ? 1 : 0

  name            = "${var.domain}-aggregate"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.aggregate[0].arn
  desired_count   = local.scaling_aggregate.min_instances
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = local.security_group_ids
    assign_public_ip = var.assign_public_ip
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.aggregate[0].arn
    }
  }

  dynamic "load_balancer" {
    for_each = var.lb_arn != null ? [1] : []
    content {
      target_group_arn = aws_lb_target_group.aggregate[0].arn
      container_name   = "grpc-gateway"
      container_port   = local.ports.grpc_gateway
    }
  }

  health_check_grace_period_seconds = var.health_check_grace_period

  tags = merge(local.tags, { "angzarr-component" = "aggregate" })
}

resource "aws_service_discovery_service" "aggregate" {
  count = var.aggregate.enabled && var.service_discovery_namespace_id != null ? 1 : 0
  name  = "${var.domain}-aggregate"

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
  tags = merge(local.tags, { "angzarr-component" = "aggregate" })
}

resource "aws_lb_target_group" "aggregate" {
  count       = var.aggregate.enabled && var.lb_arn != null ? 1 : 0
  name        = "${var.domain}-agg"
  port        = local.ports.grpc_gateway
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

  stickiness {
    type            = "lb_cookie"
    enabled         = true
    cookie_duration = 86400
  }

  tags = merge(local.tags, { "angzarr-component" = "aggregate" })
}

#------------------------------------------------------------------------------
# Process Manager Task Definition & Service
#------------------------------------------------------------------------------
resource "aws_ecs_task_definition" "process_manager" {
  count = var.process_manager.enabled ? 1 : 0

  family                   = "${var.domain}-pm"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = local.scaling_pm.cpu + 512 + 256
  memory                   = local.scaling_pm.memory + 512 + 128
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = local.task_role_arn

  container_definitions = jsonencode([
    {
      name         = "grpc-gateway"
      image        = var.images.grpc_gateway
      essential    = true
      portMappings = [{ containerPort = local.ports.grpc_gateway, protocol = "tcp" }]
      environment = [
        { name = "GRPC_BACKEND", value = "localhost:${local.ports.coordinator}" },
        { name = "PORT", value = tostring(local.ports.grpc_gateway) }
      ]
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.process_manager[0].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "grpc-gateway"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "wget -q --spider http://localhost:${local.ports.grpc_gateway}/health || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "coordinator"
      image     = var.images.coordinator_pm
      essential = true
      environment = concat(
        local.base_coordinator_env_list,
        [
          { name = "PORT", value = tostring(local.ports.coordinator) },
          { name = "COMPONENT_TYPE", value = "process_manager" },
          { name = "ANGZARR__TARGET__ADDRESS", value = "localhost:${local.ports.logic}" },
          { name = "SOURCE_DOMAINS", value = join(",", var.process_manager.source_domains) }
        ]
      )
      secrets = local.coordinator_secrets_list
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.process_manager[0].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "coordinator"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.coordinator} || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "logic"
      image     = var.images.logic
      essential = true
      environment = concat(
        [for k, v in var.process_manager.env : { name = k, value = v }],
        [
          { name = "RUST_LOG", value = var.log_level },
          { name = "PORT", value = tostring(local.ports.logic) }
        ]
      )
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.process_manager[0].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "logic"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.logic} || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    }
  ])

  tags = merge(local.tags, { "angzarr-component" = "process-manager" })
}

resource "aws_ecs_service" "process_manager" {
  count = var.process_manager.enabled ? 1 : 0

  name            = "${var.domain}-pm"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.process_manager[0].arn
  desired_count   = local.scaling_pm.min_instances
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = local.security_group_ids
    assign_public_ip = var.assign_public_ip
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.process_manager[0].arn
    }
  }

  health_check_grace_period_seconds = var.health_check_grace_period

  tags = merge(local.tags, { "angzarr-component" = "process-manager" })
}

resource "aws_service_discovery_service" "process_manager" {
  count = var.process_manager.enabled && var.service_discovery_namespace_id != null ? 1 : 0
  name  = "${var.domain}-pm"

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
  tags = merge(local.tags, { "angzarr-component" = "process-manager" })
}

#------------------------------------------------------------------------------
# Saga Task Definitions & Services
#------------------------------------------------------------------------------
resource "aws_ecs_task_definition" "saga" {
  for_each = var.sagas

  family                   = "saga-${var.domain}-${each.key}"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = local.scaling_sagas[each.key].cpu + 512 + 256
  memory                   = local.scaling_sagas[each.key].memory + 512 + 128
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = local.task_role_arn

  container_definitions = jsonencode([
    {
      name         = "grpc-gateway"
      image        = var.images.grpc_gateway
      essential    = true
      portMappings = [{ containerPort = local.ports.grpc_gateway, protocol = "tcp" }]
      environment = [
        { name = "GRPC_BACKEND", value = "localhost:${local.ports.coordinator}" },
        { name = "PORT", value = tostring(local.ports.grpc_gateway) }
      ]
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.saga[each.key].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "grpc-gateway"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "wget -q --spider http://localhost:${local.ports.grpc_gateway}/health || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "coordinator"
      image     = var.images.coordinator_saga
      essential = true
      environment = concat(
        local.base_coordinator_env_list,
        [
          { name = "PORT", value = tostring(local.ports.coordinator) },
          { name = "COMPONENT_TYPE", value = "saga" },
          { name = "ANGZARR__TARGET__ADDRESS", value = "localhost:${local.ports.logic}" },
          { name = "TARGET_DOMAIN", value = each.value.target_domain }
        ]
      )
      secrets = local.coordinator_secrets_list
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.saga[each.key].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "coordinator"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.coordinator} || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "logic"
      image     = lookup(var.images.saga_logic, each.key, var.images.logic)
      essential = true
      environment = concat(
        [for k, v in each.value.env : { name = k, value = v }],
        [
          { name = "RUST_LOG", value = var.log_level },
          { name = "PORT", value = tostring(local.ports.logic) }
        ]
      )
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.saga[each.key].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "logic"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.logic} || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    }
  ])

  tags = merge(local.tags, {
    "angzarr-component"     = "saga"
    "angzarr-target-domain" = each.value.target_domain
  })
}

resource "aws_ecs_service" "saga" {
  for_each = var.sagas

  name            = "saga-${var.domain}-${each.key}"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.saga[each.key].arn
  desired_count   = local.scaling_sagas[each.key].min_instances
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = local.security_group_ids
    assign_public_ip = var.assign_public_ip
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.saga[each.key].arn
    }
  }

  health_check_grace_period_seconds = var.health_check_grace_period

  tags = merge(local.tags, {
    "angzarr-component"     = "saga"
    "angzarr-target-domain" = each.value.target_domain
  })
}

resource "aws_service_discovery_service" "saga" {
  for_each = var.service_discovery_namespace_id != null ? var.sagas : {}
  name     = "saga-${var.domain}-${each.key}"

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
  tags = merge(local.tags, {
    "angzarr-component"     = "saga"
    "angzarr-target-domain" = each.value.target_domain
  })
}

#------------------------------------------------------------------------------
# Projector Task Definitions & Services
#------------------------------------------------------------------------------
resource "aws_ecs_task_definition" "projector" {
  for_each = var.projectors

  family                   = "projector-${var.domain}-${each.key}"
  requires_compatibilities = ["FARGATE"]
  network_mode             = "awsvpc"
  cpu                      = local.scaling_projectors[each.key].cpu + 512 + 256
  memory                   = local.scaling_projectors[each.key].memory + 512 + 128
  execution_role_arn       = var.execution_role_arn
  task_role_arn            = local.task_role_arn

  container_definitions = jsonencode([
    {
      name         = "grpc-gateway"
      image        = var.images.grpc_gateway
      essential    = true
      portMappings = [{ containerPort = local.ports.grpc_gateway, protocol = "tcp" }]
      environment = [
        { name = "GRPC_BACKEND", value = "localhost:${local.ports.coordinator}" },
        { name = "PORT", value = tostring(local.ports.grpc_gateway) }
      ]
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.projector[each.key].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "grpc-gateway"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "wget -q --spider http://localhost:${local.ports.grpc_gateway}/health || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "coordinator"
      image     = var.images.coordinator_projector
      essential = true
      environment = concat(
        local.base_coordinator_env_list,
        [
          { name = "PORT", value = tostring(local.ports.coordinator) },
          { name = "COMPONENT_TYPE", value = "projector" },
          { name = "ANGZARR__TARGET__ADDRESS", value = "localhost:${local.ports.logic}" }
        ]
      )
      secrets = local.coordinator_secrets_list
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.projector[each.key].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "coordinator"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.coordinator} || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    },
    {
      name      = "logic"
      image     = lookup(var.images.projector_logic, each.key, var.images.logic)
      essential = true
      environment = concat(
        [for k, v in each.value.env : { name = k, value = v }],
        [
          { name = "RUST_LOG", value = var.log_level },
          { name = "PORT", value = tostring(local.ports.logic) }
        ]
      )
      logConfiguration = {
        logDriver = "awslogs"
        options = {
          "awslogs-group"         = aws_cloudwatch_log_group.projector[each.key].name
          "awslogs-region"        = data.aws_region.current.name
          "awslogs-stream-prefix" = "logic"
        }
      }
      healthCheck = {
        command     = ["CMD-SHELL", "/bin/grpc_health_probe -addr=:${local.ports.logic} || exit 1"]
        interval    = 30
        timeout     = 5
        retries     = 3
        startPeriod = 60
      }
    }
  ])

  tags = merge(local.tags, {
    "angzarr-component" = "projector"
    "projector-name"    = each.key
  })
}

resource "aws_ecs_service" "projector" {
  for_each = var.projectors

  name            = "projector-${var.domain}-${each.key}"
  cluster         = var.cluster_arn
  task_definition = aws_ecs_task_definition.projector[each.key].arn
  desired_count   = local.scaling_projectors[each.key].min_instances
  launch_type     = "FARGATE"

  network_configuration {
    subnets          = var.subnet_ids
    security_groups  = local.security_group_ids
    assign_public_ip = var.assign_public_ip
  }

  dynamic "service_registries" {
    for_each = var.service_discovery_namespace_id != null ? [1] : []
    content {
      registry_arn = aws_service_discovery_service.projector[each.key].arn
    }
  }

  health_check_grace_period_seconds = var.health_check_grace_period

  tags = merge(local.tags, {
    "angzarr-component" = "projector"
    "projector-name"    = each.key
  })
}

resource "aws_service_discovery_service" "projector" {
  for_each = var.service_discovery_namespace_id != null ? var.projectors : {}
  name     = "projector-${var.domain}-${each.key}"

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
  tags = merge(local.tags, {
    "angzarr-component" = "projector"
    "projector-name"    = each.key
  })
}

#------------------------------------------------------------------------------
# Data Sources
#------------------------------------------------------------------------------
data "aws_region" "current" {}
