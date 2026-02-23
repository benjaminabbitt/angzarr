# Stack Module - Main
# Routes to platform-specific modules based on compute_type

terraform {
  required_version = ">= 1.0"

  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = ">= 2.0"
    }
    google = {
      source  = "hashicorp/google"
      version = ">= 5.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
  }
}

#------------------------------------------------------------------------------
# Kubernetes Deployments (Helm-based)
#------------------------------------------------------------------------------

module "k8s_domain" {
  source   = "../k8s-domain"
  for_each = var.compute.compute_type == "kubernetes" ? var.domains : {}

  domain    = each.key
  namespace = var.compute.namespace

  aggregate  = each.value.aggregate
  sagas      = each.value.sagas
  projectors = each.value.projectors

  storage = local.domain_storage[each.key]
  bus     = var.bus

  coordinator_images = var.coordinator_images
  labels             = local.common_labels
}

module "k8s_pm" {
  source   = "../k8s-pm"
  for_each = var.compute.compute_type == "kubernetes" ? var.process_managers : {}

  name      = each.key
  namespace = var.compute.namespace

  image         = each.value.image
  subscriptions = each.value.subscriptions
  targets       = each.value.targets
  env           = each.value.env

  storage = local.pm_storage[each.key]
  bus     = var.bus

  coordinator_images = var.coordinator_images
  labels             = local.common_labels
}

#------------------------------------------------------------------------------
# Cloud Run Deployments (GCP)
#------------------------------------------------------------------------------

module "cloudrun_domain" {
  source   = "../cloudrun-domain"
  for_each = var.compute.compute_type == "cloudrun" ? var.domains : {}

  domain     = each.key
  project_id = var.compute.project_id
  region     = var.compute.region

  aggregate  = each.value.aggregate
  sagas      = each.value.sagas
  projectors = each.value.projectors

  storage = local.domain_storage[each.key]
  bus     = var.bus

  coordinator_images = var.coordinator_images
  labels             = local.common_labels
  service_account    = var.compute.service_account
}

module "cloudrun_pm" {
  source   = "../cloudrun-pm"
  for_each = var.compute.compute_type == "cloudrun" ? var.process_managers : {}

  name       = each.key
  project_id = var.compute.project_id
  region     = var.compute.region

  image         = each.value.image
  subscriptions = each.value.subscriptions
  targets       = each.value.targets
  env           = each.value.env

  storage = local.pm_storage[each.key]
  bus     = var.bus

  coordinator_images = var.coordinator_images
  labels             = local.common_labels
  service_account    = var.compute.service_account
}

#------------------------------------------------------------------------------
# Fargate Deployments (AWS)
#------------------------------------------------------------------------------

module "fargate_domain" {
  source   = "../fargate-domain"
  for_each = var.compute.compute_type == "fargate" ? var.domains : {}

  domain      = each.key
  cluster_arn = var.compute.cluster_arn
  vpc_id      = var.compute.vpc_id
  subnet_ids  = var.compute.subnet_ids
  region      = var.compute.region

  aggregate  = each.value.aggregate
  sagas      = each.value.sagas
  projectors = each.value.projectors

  storage = local.domain_storage[each.key]
  bus     = var.bus

  coordinator_images = var.coordinator_images
  labels             = local.common_labels
  execution_role_arn = var.compute.execution_role_arn
  task_role_arn      = var.compute.task_role_arn
  log_group          = var.compute.log_group
}

module "fargate_pm" {
  source   = "../fargate-pm"
  for_each = var.compute.compute_type == "fargate" ? var.process_managers : {}

  name        = each.key
  cluster_arn = var.compute.cluster_arn
  vpc_id      = var.compute.vpc_id
  subnet_ids  = var.compute.subnet_ids
  region      = var.compute.region

  image         = each.value.image
  subscriptions = each.value.subscriptions
  targets       = each.value.targets
  env           = each.value.env

  storage = local.pm_storage[each.key]
  bus     = var.bus

  coordinator_images = var.coordinator_images
  labels             = local.common_labels
  execution_role_arn = var.compute.execution_role_arn
  task_role_arn      = var.compute.task_role_arn
  log_group          = var.compute.log_group
}
