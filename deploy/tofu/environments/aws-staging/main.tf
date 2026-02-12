# AWS Staging Environment
# Deploys angzarr to AWS Fargate
#
# This configuration mirrors the GCP Cloud Run staging environment.

terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  # Configure your backend here
  # backend "s3" {
  #   bucket = "your-terraform-state-bucket"
  #   key    = "angzarr/staging/terraform.tfstate"
  #   region = "us-east-1"
  # }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "angzarr"
      Environment = var.environment
      ManagedBy   = "opentofu"
    }
  }
}

locals {
  name = "angzarr"

  # Shared coordinator environment variables
  coordinator_env = {
    "DATABASE_URI"  = var.database_uri
    "MESSAGING_URI" = var.messaging_uri
  }
}

#------------------------------------------------------------------------------
# Base Infrastructure
#------------------------------------------------------------------------------
module "base" {
  source = "../../modules/fargate-base"

  name        = local.name
  environment = var.environment

  vpc_cidr           = "10.0.0.0/16"
  availability_zones = ["${var.aws_region}a", "${var.aws_region}b"]

  create_alb               = true
  create_service_discovery = true
}

#------------------------------------------------------------------------------
# ECR Repositories
#------------------------------------------------------------------------------
module "ecr" {
  source = "../../modules/fargate-ecr"

  name_prefix = local.name
}

#------------------------------------------------------------------------------
# Domain: Order
#------------------------------------------------------------------------------
module "order" {
  source = "../../modules/fargate-domain"

  domain      = "order"
  cluster_arn = module.base.cluster_arn
  vpc_id      = module.base.vpc_id
  subnet_ids  = module.base.private_subnet_ids

  security_group_ids             = [module.base.tasks_security_group_id]
  execution_role_arn             = module.base.execution_role_arn
  service_discovery_namespace_id = module.base.service_discovery_namespace_id
  lb_arn                         = module.base.lb_arn

  aggregate = {
    enabled       = true
    min_instances = 1
    max_instances = 5
    resources     = { cpu = "1", memory = "512Mi" }
  }

  sagas = {
    fulfillment = {
      target_domain = "fulfillment"
      min_instances = 1
      max_instances = 5
    }
  }

  images = {
    grpc_gateway          = "${module.ecr.images.grpc_gateway}:${var.image_tag}"
    coordinator_aggregate = "${module.ecr.images.coordinator_aggregate}:${var.image_tag}"
    coordinator_saga      = "${module.ecr.images.coordinator_saga}:${var.image_tag}"
    coordinator_projector = "${module.ecr.images.coordinator_projector}:${var.image_tag}"
    coordinator_pm        = "${module.ecr.images.coordinator_pm}:${var.image_tag}"
    logic                 = "${module.ecr.registry_url}/order-logic:${var.image_tag}"
    saga_logic = {
      fulfillment = "${module.ecr.registry_url}/saga-order-fulfillment:${var.image_tag}"
    }
  }

  discovery_env   = module.registry.discovery_env
  coordinator_env = local.coordinator_env
  log_level       = var.log_level
}

#------------------------------------------------------------------------------
# Domain: Inventory
#------------------------------------------------------------------------------
module "inventory" {
  source = "../../modules/fargate-domain"

  domain      = "inventory"
  cluster_arn = module.base.cluster_arn
  vpc_id      = module.base.vpc_id
  subnet_ids  = module.base.private_subnet_ids

  security_group_ids             = [module.base.tasks_security_group_id]
  execution_role_arn             = module.base.execution_role_arn
  service_discovery_namespace_id = module.base.service_discovery_namespace_id
  lb_arn                         = module.base.lb_arn

  aggregate = {
    enabled       = true
    min_instances = 1
    max_instances = 5
    resources     = { cpu = "1", memory = "512Mi" }
  }

  images = {
    grpc_gateway          = "${module.ecr.images.grpc_gateway}:${var.image_tag}"
    coordinator_aggregate = "${module.ecr.images.coordinator_aggregate}:${var.image_tag}"
    coordinator_saga      = "${module.ecr.images.coordinator_saga}:${var.image_tag}"
    coordinator_projector = "${module.ecr.images.coordinator_projector}:${var.image_tag}"
    coordinator_pm        = "${module.ecr.images.coordinator_pm}:${var.image_tag}"
    logic                 = "${module.ecr.registry_url}/inventory-logic:${var.image_tag}"
  }

  discovery_env   = module.registry.discovery_env
  coordinator_env = local.coordinator_env
  log_level       = var.log_level
}

#------------------------------------------------------------------------------
# Domain: Fulfillment
#------------------------------------------------------------------------------
module "fulfillment" {
  source = "../../modules/fargate-domain"

  domain      = "fulfillment"
  cluster_arn = module.base.cluster_arn
  vpc_id      = module.base.vpc_id
  subnet_ids  = module.base.private_subnet_ids

  security_group_ids             = [module.base.tasks_security_group_id]
  execution_role_arn             = module.base.execution_role_arn
  service_discovery_namespace_id = module.base.service_discovery_namespace_id
  lb_arn                         = module.base.lb_arn

  aggregate = {
    enabled       = true
    min_instances = 1
    max_instances = 5
    resources     = { cpu = "1", memory = "512Mi" }
  }

  sagas = {
    inventory = {
      target_domain = "inventory"
      min_instances = 1
      max_instances = 5
    }
  }

  images = {
    grpc_gateway          = "${module.ecr.images.grpc_gateway}:${var.image_tag}"
    coordinator_aggregate = "${module.ecr.images.coordinator_aggregate}:${var.image_tag}"
    coordinator_saga      = "${module.ecr.images.coordinator_saga}:${var.image_tag}"
    coordinator_projector = "${module.ecr.images.coordinator_projector}:${var.image_tag}"
    coordinator_pm        = "${module.ecr.images.coordinator_pm}:${var.image_tag}"
    logic                 = "${module.ecr.registry_url}/fulfillment-logic:${var.image_tag}"
    saga_logic = {
      inventory = "${module.ecr.registry_url}/saga-fulfillment-inventory:${var.image_tag}"
    }
  }

  discovery_env   = module.registry.discovery_env
  coordinator_env = local.coordinator_env
  log_level       = var.log_level
}

#------------------------------------------------------------------------------
# Infrastructure Services
#------------------------------------------------------------------------------
module "infrastructure" {
  source = "../../modules/fargate-infrastructure"

  cluster_arn = module.base.cluster_arn
  vpc_id      = module.base.vpc_id
  subnet_ids  = module.base.private_subnet_ids

  security_group_ids             = [module.base.tasks_security_group_id]
  execution_role_arn             = module.base.execution_role_arn
  service_discovery_namespace_id = module.base.service_discovery_namespace_id
  lb_arn                         = module.base.lb_arn

  stream = {
    enabled       = true
    image         = "${module.ecr.images.stream}:${var.image_tag}"
    min_instances = 1
    max_instances = 3
  }

  topology = {
    enabled       = true
    image         = "${module.ecr.images.topology}:${var.image_tag}"
    min_instances = 1
    max_instances = 2
  }

  coordinator_env = local.coordinator_env
  log_level       = var.log_level
}

#------------------------------------------------------------------------------
# Service Discovery Registry
#------------------------------------------------------------------------------
module "registry" {
  source = "../../modules/fargate-registry"

  namespace_name = module.base.service_discovery_namespace_name

  services = merge(
    module.order.discovery_entries,
    module.inventory.discovery_entries,
    module.fulfillment.discovery_entries
  )

  stream_dns   = module.infrastructure.service_discovery_dns.stream
  topology_dns = module.infrastructure.service_discovery_dns.topology
}
