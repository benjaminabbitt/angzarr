# Infrastructure Module: PostgreSQL - Variables

variable "name" {
  description = "Release name for PostgreSQL"
  type        = string
  default     = "angzarr-db"
}

variable "image" {
  description = "PostgreSQL container image"
  type        = string
  default     = "postgres:16"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "username" {
  description = "PostgreSQL username"
  type        = string
  default     = "angzarr"
}

variable "password" {
  description = "PostgreSQL user password (generated if empty)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "admin_password" {
  description = "PostgreSQL admin password (generated if empty)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "database" {
  description = "PostgreSQL database name"
  type        = string
  default     = "angzarr"
}

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

variable "storage_class" {
  description = "Storage class for PVC"
  type        = string
  default     = ""
}

variable "resources" {
  description = "Resource limits and requests"
  type = object({
    limits = optional(object({
      cpu    = optional(string, "500m")
      memory = optional(string, "512Mi")
    }), {})
    requests = optional(object({
      cpu    = optional(string, "100m")
      memory = optional(string, "256Mi")
    }), {})
  })
  default = {}
}
