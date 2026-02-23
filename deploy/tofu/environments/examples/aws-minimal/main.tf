# Example: AWS Minimal Stack
# Fargate + SNS/SQS + DynamoDB (serverless, no VPC required)
#
# Prerequisites:
# - AWS credentials configured
# - Terraform/OpenTofu installed

terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

#------------------------------------------------------------------------------
# AWS Provider
#------------------------------------------------------------------------------

provider "aws" {
  region = var.region
}

variable "region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

#------------------------------------------------------------------------------
# Infrastructure
#------------------------------------------------------------------------------

module "sns_sqs" {
  source = "../../../modules/sns-sqs"

  name_prefix = "angzarr-poker"
  domains     = ["player", "table", "hand"]

  subscribers = {
    "saga-table-hand" = ["table"]
    "saga-hand-table" = ["hand"]
  }
}

module "dynamo" {
  source = "../../../modules/dynamo"

  table_prefix = "angzarr-poker"
  billing_mode = "PAY_PER_REQUEST"

  point_in_time_recovery = false # For demo; enable in production
}

#------------------------------------------------------------------------------
# Stack
#------------------------------------------------------------------------------

module "stack" {
  source = "../../../modules/stack"

  name = "poker"

  compute = {
    compute_type = "fargate"
    region       = var.region
    # Note: For a real deployment, you'd need:
    # cluster_arn, vpc_id, subnet_ids, execution_role_arn, log_group
  }

  bus = module.sns_sqs.bus

  default_storage = {
    event_store    = module.dynamo.event_store
    position_store = module.dynamo.position_store
    snapshot_store = null # No snapshots for this minimal example
  }

  coordinator_images = {
    aggregate    = "ghcr.io/angzarr-io/coordinator-aggregate:latest"
    saga         = "ghcr.io/angzarr-io/coordinator-saga:latest"
    projector    = "ghcr.io/angzarr-io/coordinator-projector:latest"
    pm           = "ghcr.io/angzarr-io/coordinator-pm:latest"
    grpc_gateway = null
  }

  domains = {
    player = {
      aggregate = {
        image = "ghcr.io/angzarr-io/player-aggregate:latest"
      }
    }
    table = {
      aggregate = {
        image = "ghcr.io/angzarr-io/table-aggregate:latest"
      }
      sagas = {
        hand = {
          target_domain = "hand"
          image         = "ghcr.io/angzarr-io/saga-table-hand:latest"
        }
      }
    }
    hand = {
      aggregate = {
        image = "ghcr.io/angzarr-io/hand-aggregate:latest"
      }
      sagas = {
        table = {
          target_domain = "table"
          image         = "ghcr.io/angzarr-io/saga-hand-table:latest"
        }
      }
    }
  }

  process_managers = {}
}

#------------------------------------------------------------------------------
# Outputs
#------------------------------------------------------------------------------

output "topology_mermaid" {
  value = module.stack.topology_mermaid
}

output "dynamo_tables" {
  value = module.dynamo.table_names
}

output "sns_topics" {
  value = module.sns_sqs.topic_arns
}
