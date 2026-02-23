# Infrastructure Module: RabbitMQ
# Deploys RabbitMQ via Helm and outputs connection info

terraform {
  required_providers {
    helm = {
      source  = "hashicorp/helm"
      version = ">= 2.0"
    }
    random = {
      source  = "hashicorp/random"
      version = ">= 3.0"
    }
  }
}

resource "random_password" "rabbitmq" {
  count   = var.password == "" ? 1 : 0
  length  = 24
  special = false
}

resource "random_password" "erlang_cookie" {
  count   = var.erlang_cookie == "" ? 1 : 0
  length  = 32
  special = false
}

locals {
  password      = var.password != "" ? var.password : random_password.rabbitmq[0].result
  erlang_cookie = var.erlang_cookie != "" ? var.erlang_cookie : random_password.erlang_cookie[0].result
  service_name  = "${var.name}-rabbitmq"
}

resource "helm_release" "rabbitmq" {
  name      = var.name
  namespace = var.namespace
  chart     = "${path.module}/../../../helm/angzarr-mq-rabbitmq"

  values = [yamlencode({
    rabbitmq = {
      auth = {
        username     = var.username
        password     = local.password
        erlangCookie = local.erlang_cookie
      }
      persistence = {
        enabled      = var.persistence_enabled
        size         = var.persistence_size
        storageClass = var.storage_class
      }
      resources = var.resources
    }
  })]
}
