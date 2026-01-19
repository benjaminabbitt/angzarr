# Database module variables

variable "type" {
  description = "Database type: postgresql or mongodb"
  type        = string
  validation {
    condition     = contains(["postgresql", "mongodb"], var.type)
    error_message = "Database type must be 'postgresql' or 'mongodb'."
  }
}

variable "managed" {
  description = "Use cloud-managed database instead of Helm chart"
  type        = bool
  default     = false
}

variable "release_name" {
  description = "Helm release name"
  type        = string
  default     = "angzarr-db"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
  default     = "angzarr"
}

variable "postgresql_chart_version" {
  description = "PostgreSQL Helm chart version"
  type        = string
  default     = "18.2.0"
}

variable "mongodb_chart_version" {
  description = "MongoDB Helm chart version"
  type        = string
  default     = "16.4.0"
}

# Authentication
variable "admin_password" {
  description = "Admin/root password (auto-generated if not provided)"
  type        = string
  default     = null
  sensitive   = true
}

variable "username" {
  description = "Application database username"
  type        = string
  default     = "angzarr"
}

variable "password" {
  description = "Application database password (auto-generated if not provided)"
  type        = string
  default     = null
  sensitive   = true
}

variable "database" {
  description = "Database name"
  type        = string
  default     = "angzarr"
}

# Persistence
variable "persistence_enabled" {
  description = "Enable persistent storage"
  type        = bool
  default     = true
}

variable "persistence_size" {
  description = "Persistent volume size"
  type        = string
  default     = "8Gi"
}

# Resources
variable "resources" {
  description = "Resource requests and limits"
  type = object({
    requests = object({
      memory = string
      cpu    = string
    })
    limits = object({
      memory = string
      cpu    = string
    })
  })
  default = {
    requests = {
      memory = "256Mi"
      cpu    = "100m"
    }
    limits = {
      memory = "512Mi"
      cpu    = "500m"
    }
  }
}

variable "metrics_enabled" {
  description = "Enable Prometheus metrics"
  type        = bool
  default     = true
}

# External/managed database connection (when managed = true)
variable "external_host" {
  description = "External database host (for managed databases)"
  type        = string
  default     = ""
}

variable "external_port" {
  description = "External database port (for managed databases)"
  type        = number
  default     = 0
}

variable "external_uri" {
  description = "External database URI (for managed databases)"
  type        = string
  default     = ""
  sensitive   = true
}
