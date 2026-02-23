# MemoryDB Module - Main
# AWS MemoryDB for Redis (snapshot store)

terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
    random = {
      source  = "hashicorp/random"
      version = ">= 3.0"
    }
  }
}

locals {
  tags = merge(var.tags, {
    "angzarr-component" = "storage"
    "angzarr-storage"   = "memorydb"
  })
}

#------------------------------------------------------------------------------
# Random Password
#------------------------------------------------------------------------------

resource "random_password" "auth" {
  count   = var.auth_token == null ? 1 : 0
  length  = 32
  special = false
}

locals {
  auth_token = var.auth_token != null ? var.auth_token : random_password.auth[0].result
}

#------------------------------------------------------------------------------
# Subnet Group
#------------------------------------------------------------------------------

resource "aws_memorydb_subnet_group" "angzarr" {
  name       = "${var.cluster_name}-subnet-group"
  subnet_ids = var.subnet_ids

  tags = local.tags
}

#------------------------------------------------------------------------------
# Security Group
#------------------------------------------------------------------------------

resource "aws_security_group" "memorydb" {
  name        = "${var.cluster_name}-memorydb-sg"
  description = "Security group for MemoryDB"
  vpc_id      = var.vpc_id

  ingress {
    description     = "Redis from allowed security groups"
    from_port       = 6379
    to_port         = 6379
    protocol        = "tcp"
    security_groups = var.allowed_security_group_ids
  }

  egress {
    description = "Allow all outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = local.tags
}

#------------------------------------------------------------------------------
# ACL
#------------------------------------------------------------------------------

resource "aws_memorydb_acl" "angzarr" {
  name = "${var.cluster_name}-acl"

  user_names = [aws_memorydb_user.angzarr.name]

  tags = local.tags
}

resource "aws_memorydb_user" "angzarr" {
  user_name     = "angzarr"
  access_string = "on ~* &* +@all"

  authentication_mode {
    type      = "password"
    passwords = [local.auth_token]
  }

  tags = local.tags
}

#------------------------------------------------------------------------------
# Parameter Group
#------------------------------------------------------------------------------

resource "aws_memorydb_parameter_group" "angzarr" {
  name   = "${var.cluster_name}-params"
  family = "memorydb_redis7"

  dynamic "parameter" {
    for_each = var.parameters
    content {
      name  = parameter.value.name
      value = parameter.value.value
    }
  }

  tags = local.tags
}

#------------------------------------------------------------------------------
# Cluster
#------------------------------------------------------------------------------

resource "aws_memorydb_cluster" "angzarr" {
  name                 = var.cluster_name
  node_type            = var.node_type
  num_shards           = var.num_shards
  num_replicas_per_shard = var.num_replicas_per_shard

  acl_name                 = aws_memorydb_acl.angzarr.name
  parameter_group_name     = aws_memorydb_parameter_group.angzarr.name
  subnet_group_name        = aws_memorydb_subnet_group.angzarr.name
  security_group_ids       = [aws_security_group.memorydb.id]

  tls_enabled              = var.tls_enabled
  kms_key_arn              = var.kms_key_arn

  snapshot_retention_limit = var.snapshot_retention_limit
  snapshot_window          = var.snapshot_window
  maintenance_window       = var.maintenance_window

  auto_minor_version_upgrade = var.auto_minor_version_upgrade

  tags = local.tags
}

#------------------------------------------------------------------------------
# Secrets Manager
#------------------------------------------------------------------------------

resource "aws_secretsmanager_secret" "credentials" {
  name = "${var.cluster_name}-credentials"

  tags = local.tags
}

resource "aws_secretsmanager_secret_version" "credentials" {
  secret_id = aws_secretsmanager_secret.credentials.id
  secret_string = jsonencode({
    username = "angzarr"
    password = local.auth_token
    host     = aws_memorydb_cluster.angzarr.cluster_endpoint[0].address
    port     = aws_memorydb_cluster.angzarr.cluster_endpoint[0].port
  })
}
