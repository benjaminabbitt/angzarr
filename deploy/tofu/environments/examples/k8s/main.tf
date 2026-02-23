# Example: Kubernetes Stack
# Works on any K8s cluster (Kind, GKE, EKS, AKS, OpenShift, etc.)
#
# Prerequisites:
# - Kubernetes cluster with kubectl configured
# - Helm 3.x installed

terraform {
  required_version = ">= 1.0"

  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.0"
    }
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
  }
}

#------------------------------------------------------------------------------
# Kubernetes Provider
#------------------------------------------------------------------------------

provider "kubernetes" {
  # Uses KUBECONFIG by default
}

provider "helm" {
  kubernetes {
    # Uses KUBECONFIG by default
  }
}

#------------------------------------------------------------------------------
# Infrastructure (Helm Charts)
#------------------------------------------------------------------------------

module "rabbit" {
  source = "../../../modules/infra-rabbit"

  namespace    = "angzarr"
  release_name = "angzarr-rabbit"
}

module "postgres" {
  source = "../../../modules/infra-postgres"

  namespace    = "angzarr"
  release_name = "angzarr-postgres"
}

module "redis" {
  source = "../../../modules/infra-redis"

  namespace    = "angzarr"
  release_name = "angzarr-redis"
}

#------------------------------------------------------------------------------
# Stack
#------------------------------------------------------------------------------

module "stack" {
  source = "../../../modules/stack"

  name = "poker"

  compute = {
    compute_type = "kubernetes"
    namespace    = "angzarr"
  }

  bus = module.rabbit.bus

  default_storage = {
    event_store = {
      connection_uri = module.postgres.connection_uri
      provides = {
        capabilities  = ["event_store", "position_store", "transactions"]
        rust_features = ["postgres"]
      }
    }
    position_store = {
      connection_uri = module.postgres.connection_uri
      provides = {
        capabilities  = ["position_store", "transactions"]
        rust_features = ["postgres"]
      }
    }
    snapshot_store = {
      connection_uri = module.redis.connection_uri
      provides = {
        capabilities  = ["snapshot_store"]
        rust_features = ["redis"]
      }
    }
  }

  coordinator_images = {
    aggregate    = "ghcr.io/angzarr-io/coordinator-aggregate:latest"
    saga         = "ghcr.io/angzarr-io/coordinator-saga:latest"
    projector    = "ghcr.io/angzarr-io/coordinator-projector:latest"
    pm           = "ghcr.io/angzarr-io/coordinator-pm:latest"
    grpc_gateway = null
  }

  # Example poker domains
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
  description = "Mermaid diagram of domain topology"
  value       = module.stack.topology_mermaid
}

output "entry_points" {
  description = "External entry point domains"
  value       = module.stack.entry_points
}

output "rust_features" {
  description = "Required Rust features"
  value       = module.stack.rust_features
}
