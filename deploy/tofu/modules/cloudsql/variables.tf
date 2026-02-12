# Cloud SQL Module - Variables

# Required
variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region for the instance"
  type        = string
}

variable "instance_name" {
  description = "Name of the Cloud SQL instance"
  type        = string
}

# Database configuration
variable "database" {
  description = "Name of the database to create"
  type        = string
  default     = "angzarr"
}

variable "username" {
  description = "Database username"
  type        = string
  default     = "angzarr"
}

variable "password" {
  description = "Database password (auto-generated if not provided)"
  type        = string
  default     = null
  sensitive   = true
}

variable "database_version" {
  description = "PostgreSQL version"
  type        = string
  default     = "POSTGRES_16"
}

# Instance sizing
variable "tier" {
  description = "Machine tier (db-f1-micro, db-g1-small, db-custom-CPU-RAM)"
  type        = string
  default     = "db-f1-micro"
}

variable "availability_type" {
  description = "Availability type: ZONAL or REGIONAL (HA)"
  type        = string
  default     = "ZONAL"
}

variable "disk_type" {
  description = "Disk type: PD_SSD or PD_HDD"
  type        = string
  default     = "PD_SSD"
}

variable "disk_size" {
  description = "Disk size in GB"
  type        = number
  default     = 10
}

variable "disk_autoresize" {
  description = "Enable automatic disk resize"
  type        = bool
  default     = true
}

# Networking
variable "enable_public_ip" {
  description = "Enable public IP address"
  type        = bool
  default     = false
}

variable "enable_private_ip" {
  description = "Enable private IP address (requires VPC)"
  type        = bool
  default     = true
}

variable "vpc_network" {
  description = "VPC network self-link for private IP"
  type        = string
  default     = null
}

variable "require_ssl" {
  description = "Require SSL for connections"
  type        = bool
  default     = true
}

variable "authorized_networks" {
  description = "Authorized networks for public IP access"
  type = list(object({
    name = string
    cidr = string
  }))
  default = []
}

# Backup
variable "backup_enabled" {
  description = "Enable automated backups"
  type        = bool
  default     = true
}

variable "backup_start_time" {
  description = "Backup start time (HH:MM format, UTC)"
  type        = string
  default     = "03:00"
}

variable "point_in_time_recovery" {
  description = "Enable point-in-time recovery"
  type        = bool
  default     = true
}

variable "backup_retained_count" {
  description = "Number of backups to retain"
  type        = number
  default     = 7
}

# Maintenance
variable "maintenance_window_day" {
  description = "Day of week for maintenance (1=Mon, 7=Sun)"
  type        = number
  default     = 7
}

variable "maintenance_window_hour" {
  description = "Hour for maintenance (0-23 UTC)"
  type        = number
  default     = 4
}

variable "maintenance_update_track" {
  description = "Maintenance update track: canary or stable"
  type        = string
  default     = "stable"
}

# Monitoring
variable "query_insights_enabled" {
  description = "Enable Query Insights"
  type        = bool
  default     = true
}

# Database flags
variable "database_flags" {
  description = "PostgreSQL database flags"
  type        = map(string)
  default = {
    "log_min_duration_statement" = "1000" # Log queries > 1s
  }
}

# Protection
variable "deletion_protection" {
  description = "Enable deletion protection"
  type        = bool
  default     = true
}

# Secrets
variable "create_secrets" {
  description = "Create Secret Manager secrets for credentials"
  type        = bool
  default     = true
}

variable "secret_accessors" {
  description = "IAM members that can access the secrets"
  type        = list(string)
  default     = []
}

# Labels
variable "labels" {
  description = "Labels to apply to resources"
  type        = map(string)
  default     = {}
}
