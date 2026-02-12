# GCP Environment - Variables

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-central1"
}

variable "environment" {
  description = "Environment name (dev, staging, prod)"
  type        = string
  default     = "dev"
}

#------------------------------------------------------------------------------
# Database
#------------------------------------------------------------------------------
variable "database_tier" {
  description = "Cloud SQL instance tier"
  type        = string
  default     = "db-f1-micro"
}

variable "database_ha" {
  description = "Enable high availability for database"
  type        = bool
  default     = false
}

#------------------------------------------------------------------------------
# Container Images
#------------------------------------------------------------------------------
variable "image_registry" {
  description = "Container image registry (e.g., gcr.io/project-id)"
  type        = string
}

variable "image_tag" {
  description = "Container image tag"
  type        = string
  default     = "latest"
}

#------------------------------------------------------------------------------
# Features
#------------------------------------------------------------------------------
variable "enable_stream" {
  description = "Enable stream service"
  type        = bool
  default     = true
}

variable "enable_topology" {
  description = "Enable topology service"
  type        = bool
  default     = true
}

variable "allow_unauthenticated" {
  description = "Allow unauthenticated access to services"
  type        = bool
  default     = false
}
