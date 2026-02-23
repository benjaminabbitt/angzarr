# RDS PostgreSQL Module - Variables

variable "identifier" {
  description = "RDS instance identifier"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID for the RDS instance"
  type        = string
}

variable "subnet_ids" {
  description = "Subnet IDs for the DB subnet group"
  type        = list(string)
}

variable "allowed_security_group_ids" {
  description = "Security group IDs allowed to connect to RDS"
  type        = list(string)
}

#------------------------------------------------------------------------------
# Instance Configuration
#------------------------------------------------------------------------------

variable "instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.t3.micro"
}

variable "engine_version" {
  description = "PostgreSQL engine version"
  type        = string
  default     = "16"
}

variable "allocated_storage" {
  description = "Allocated storage in GB"
  type        = number
  default     = 20
}

variable "max_allocated_storage" {
  description = "Maximum allocated storage in GB (for autoscaling)"
  type        = number
  default     = 100
}

variable "storage_type" {
  description = "Storage type: gp2, gp3, io1, io2"
  type        = string
  default     = "gp3"
}

#------------------------------------------------------------------------------
# Database Configuration
#------------------------------------------------------------------------------

variable "database_name" {
  description = "Name of the database to create"
  type        = string
  default     = "angzarr"
}

variable "master_username" {
  description = "Master username"
  type        = string
  default     = "angzarr"
}

variable "master_password" {
  description = "Master password (null = auto-generate)"
  type        = string
  default     = null
  sensitive   = true
}

#------------------------------------------------------------------------------
# High Availability & Durability
#------------------------------------------------------------------------------

variable "multi_az" {
  description = "Enable Multi-AZ deployment"
  type        = bool
  default     = false
}

variable "deletion_protection" {
  description = "Enable deletion protection"
  type        = bool
  default     = false
}

variable "skip_final_snapshot" {
  description = "Skip final snapshot when destroying"
  type        = bool
  default     = true
}

variable "backup_retention_period" {
  description = "Backup retention period in days"
  type        = number
  default     = 7
}

variable "backup_window" {
  description = "Preferred backup window (UTC)"
  type        = string
  default     = "03:00-04:00"
}

variable "maintenance_window" {
  description = "Preferred maintenance window (UTC)"
  type        = string
  default     = "Mon:04:00-Mon:05:00"
}

#------------------------------------------------------------------------------
# Monitoring
#------------------------------------------------------------------------------

variable "performance_insights_enabled" {
  description = "Enable Performance Insights"
  type        = bool
  default     = false
}

variable "monitoring_interval" {
  description = "Enhanced monitoring interval in seconds (0 = disabled)"
  type        = number
  default     = 0
}

#------------------------------------------------------------------------------
# Security
#------------------------------------------------------------------------------

variable "kms_key_id" {
  description = "KMS key ID for encryption (null = AWS managed)"
  type        = string
  default     = null
}

variable "auto_minor_version_upgrade" {
  description = "Auto upgrade minor versions"
  type        = bool
  default     = true
}

#------------------------------------------------------------------------------
# Tags
#------------------------------------------------------------------------------

variable "tags" {
  description = "Tags to apply to all resources"
  type        = map(string)
  default     = {}
}
