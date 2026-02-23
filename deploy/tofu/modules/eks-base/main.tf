# EKS Base Module - Main
# AWS EKS cluster with managed node groups

terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = ">= 5.0"
    }
    tls = {
      source  = "hashicorp/tls"
      version = ">= 4.0"
    }
  }
}

data "aws_region" "current" {}
data "aws_caller_identity" "current" {}

locals {
  tags = merge(var.tags, {
    "angzarr-component" = "compute"
    "angzarr-compute"   = "eks"
  })
}

#------------------------------------------------------------------------------
# VPC (optional - create or use existing)
#------------------------------------------------------------------------------

resource "aws_vpc" "eks" {
  count = var.create_vpc ? 1 : 0

  cidr_block           = var.vpc_cidr
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = merge(local.tags, {
    Name = "${var.cluster_name}-vpc"
  })
}

resource "aws_internet_gateway" "eks" {
  count = var.create_vpc ? 1 : 0

  vpc_id = aws_vpc.eks[0].id

  tags = merge(local.tags, {
    Name = "${var.cluster_name}-igw"
  })
}

# Public subnets (for NAT gateways and load balancers)
resource "aws_subnet" "public" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  vpc_id                  = aws_vpc.eks[0].id
  cidr_block              = cidrsubnet(var.vpc_cidr, 4, count.index)
  availability_zone       = var.availability_zones[count.index]
  map_public_ip_on_launch = true

  tags = merge(local.tags, {
    Name                                        = "${var.cluster_name}-public-${var.availability_zones[count.index]}"
    "kubernetes.io/role/elb"                    = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  })
}

# Private subnets (for nodes)
resource "aws_subnet" "private" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  vpc_id            = aws_vpc.eks[0].id
  cidr_block        = cidrsubnet(var.vpc_cidr, 4, count.index + length(var.availability_zones))
  availability_zone = var.availability_zones[count.index]

  tags = merge(local.tags, {
    Name                                        = "${var.cluster_name}-private-${var.availability_zones[count.index]}"
    "kubernetes.io/role/internal-elb"           = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  })
}

# NAT Gateway (one per AZ for HA, or single for cost savings)
resource "aws_eip" "nat" {
  count = var.create_vpc ? (var.single_nat_gateway ? 1 : length(var.availability_zones)) : 0

  domain = "vpc"

  tags = merge(local.tags, {
    Name = "${var.cluster_name}-nat-${count.index}"
  })
}

resource "aws_nat_gateway" "eks" {
  count = var.create_vpc ? (var.single_nat_gateway ? 1 : length(var.availability_zones)) : 0

  allocation_id = aws_eip.nat[count.index].id
  subnet_id     = aws_subnet.public[count.index].id

  tags = merge(local.tags, {
    Name = "${var.cluster_name}-nat-${count.index}"
  })

  depends_on = [aws_internet_gateway.eks]
}

# Route tables
resource "aws_route_table" "public" {
  count = var.create_vpc ? 1 : 0

  vpc_id = aws_vpc.eks[0].id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.eks[0].id
  }

  tags = merge(local.tags, {
    Name = "${var.cluster_name}-public"
  })
}

resource "aws_route_table" "private" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  vpc_id = aws_vpc.eks[0].id

  route {
    cidr_block     = "0.0.0.0/0"
    nat_gateway_id = aws_nat_gateway.eks[var.single_nat_gateway ? 0 : count.index].id
  }

  tags = merge(local.tags, {
    Name = "${var.cluster_name}-private-${count.index}"
  })
}

resource "aws_route_table_association" "public" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  subnet_id      = aws_subnet.public[count.index].id
  route_table_id = aws_route_table.public[0].id
}

resource "aws_route_table_association" "private" {
  count = var.create_vpc ? length(var.availability_zones) : 0

  subnet_id      = aws_subnet.private[count.index].id
  route_table_id = aws_route_table.private[count.index].id
}

locals {
  vpc_id     = var.create_vpc ? aws_vpc.eks[0].id : var.vpc_id
  subnet_ids = var.create_vpc ? aws_subnet.private[*].id : var.subnet_ids
}

#------------------------------------------------------------------------------
# EKS Cluster IAM Role
#------------------------------------------------------------------------------

resource "aws_iam_role" "cluster" {
  name = "${var.cluster_name}-cluster-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "eks.amazonaws.com"
        }
      }
    ]
  })

  tags = local.tags
}

resource "aws_iam_role_policy_attachment" "cluster_policy" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSClusterPolicy"
  role       = aws_iam_role.cluster.name
}

#------------------------------------------------------------------------------
# EKS Cluster
#------------------------------------------------------------------------------

resource "aws_eks_cluster" "angzarr" {
  name     = var.cluster_name
  role_arn = aws_iam_role.cluster.arn
  version  = var.cluster_version

  vpc_config {
    subnet_ids              = local.subnet_ids
    endpoint_private_access = true
    endpoint_public_access  = var.endpoint_public_access
  }

  enabled_cluster_log_types = var.enabled_cluster_log_types

  tags = local.tags

  depends_on = [
    aws_iam_role_policy_attachment.cluster_policy,
  ]
}

#------------------------------------------------------------------------------
# OIDC Provider (for IRSA)
#------------------------------------------------------------------------------

data "tls_certificate" "eks" {
  url = aws_eks_cluster.angzarr.identity[0].oidc[0].issuer
}

resource "aws_iam_openid_connect_provider" "eks" {
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = [data.tls_certificate.eks.certificates[0].sha1_fingerprint]
  url             = aws_eks_cluster.angzarr.identity[0].oidc[0].issuer

  tags = local.tags
}

#------------------------------------------------------------------------------
# Node Group IAM Role
#------------------------------------------------------------------------------

resource "aws_iam_role" "node" {
  name = "${var.cluster_name}-node-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })

  tags = local.tags
}

resource "aws_iam_role_policy_attachment" "node_worker" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKSWorkerNodePolicy"
  role       = aws_iam_role.node.name
}

resource "aws_iam_role_policy_attachment" "node_cni" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEKS_CNI_Policy"
  role       = aws_iam_role.node.name
}

resource "aws_iam_role_policy_attachment" "node_ecr" {
  policy_arn = "arn:aws:iam::aws:policy/AmazonEC2ContainerRegistryReadOnly"
  role       = aws_iam_role.node.name
}

#------------------------------------------------------------------------------
# Managed Node Groups
#------------------------------------------------------------------------------

resource "aws_eks_node_group" "angzarr" {
  for_each = var.node_groups

  cluster_name    = aws_eks_cluster.angzarr.name
  node_group_name = each.key
  node_role_arn   = aws_iam_role.node.arn
  subnet_ids      = local.subnet_ids

  instance_types = each.value.instance_types
  capacity_type  = lookup(each.value, "capacity_type", "ON_DEMAND")

  scaling_config {
    min_size     = each.value.min_size
    max_size     = each.value.max_size
    desired_size = lookup(each.value, "desired_size", each.value.min_size)
  }

  dynamic "taint" {
    for_each = lookup(each.value, "taints", [])
    content {
      key    = taint.value.key
      value  = lookup(taint.value, "value", null)
      effect = taint.value.effect
    }
  }

  labels = lookup(each.value, "labels", {})

  tags = merge(local.tags, {
    "angzarr-node-group" = each.key
  })

  depends_on = [
    aws_iam_role_policy_attachment.node_worker,
    aws_iam_role_policy_attachment.node_cni,
    aws_iam_role_policy_attachment.node_ecr,
  ]
}

#------------------------------------------------------------------------------
# EKS Add-ons
#------------------------------------------------------------------------------

resource "aws_eks_addon" "coredns" {
  cluster_name = aws_eks_cluster.angzarr.name
  addon_name   = "coredns"

  depends_on = [aws_eks_node_group.angzarr]
}

resource "aws_eks_addon" "kube_proxy" {
  cluster_name = aws_eks_cluster.angzarr.name
  addon_name   = "kube-proxy"
}

resource "aws_eks_addon" "vpc_cni" {
  cluster_name = aws_eks_cluster.angzarr.name
  addon_name   = "vpc-cni"
}
