# Cloud Run Base Module - Variables

variable "name_prefix" {
  description = "Prefix for resource names"
  type        = string
  default     = "angzarr"
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
}

#------------------------------------------------------------------------------
# VPC Connector Configuration
#------------------------------------------------------------------------------

variable "create_vpc_connector" {
  description = "Create Serverless VPC Access connector"
  type        = bool
  default     = false
}

variable "vpc_connector_network" {
  description = "VPC network for connector (required if create_vpc_connector = true)"
  type        = string
  default     = null
}

variable "vpc_connector_ip_range" {
  description = "IP CIDR range for connector (required if create_vpc_connector = true)"
  type        = string
  default     = null
}

variable "vpc_connector_min_instances" {
  description = "Minimum connector instances"
  type        = number
  default     = 2
}

variable "vpc_connector_max_instances" {
  description = "Maximum connector instances"
  type        = number
  default     = 3
}

variable "vpc_connector_machine_type" {
  description = "Connector machine type"
  type        = string
  default     = "e2-micro"
}

#------------------------------------------------------------------------------
# IAM
#------------------------------------------------------------------------------

variable "additional_roles" {
  description = "Additional IAM roles to grant to the service account"
  type        = list(string)
  default     = []
}

#------------------------------------------------------------------------------
# Labels
#------------------------------------------------------------------------------

variable "labels" {
  description = "Labels to apply to all resources"
  type        = map(string)
  default     = {}
}
