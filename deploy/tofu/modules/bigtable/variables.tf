# Bigtable Module - Variables

variable "instance_name" {
  description = "Bigtable instance name"
  type        = string
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "zone" {
  description = "GCP zone for the cluster"
  type        = string
}

#------------------------------------------------------------------------------
# Cluster Configuration
#------------------------------------------------------------------------------

variable "num_nodes" {
  description = "Number of nodes (ignored if autoscaling is set)"
  type        = number
  default     = 1
}

variable "storage_type" {
  description = "Storage type: SSD or HDD"
  type        = string
  default     = "SSD"

  validation {
    condition     = contains(["SSD", "HDD"], var.storage_type)
    error_message = "storage_type must be SSD or HDD"
  }
}

variable "kms_key_name" {
  description = "KMS key for encryption (null = Google managed)"
  type        = string
  default     = null
}

variable "autoscaling" {
  description = <<-EOT
    Autoscaling configuration. If set, num_nodes is ignored.

    Example:
      {
        min_nodes      = 1
        max_nodes      = 10
        cpu_target     = 50
        storage_target = 8192
      }
  EOT
  type = object({
    min_nodes      = number
    max_nodes      = number
    cpu_target     = number
    storage_target = optional(number)
  })
  default = null
}

#------------------------------------------------------------------------------
# Table Configuration
#------------------------------------------------------------------------------

variable "events_gc_policy" {
  description = "GC policy for events table (null = keep all)"
  type = object({
    max_age     = optional(string)
    max_version = optional(number)
  })
  default = null
}

variable "deletion_protection" {
  description = "Enable deletion protection"
  type        = bool
  default     = false
}

#------------------------------------------------------------------------------
# IAM
#------------------------------------------------------------------------------

variable "reader_members" {
  description = "Members to grant bigtable.reader role"
  type        = list(string)
  default     = []
}

variable "writer_members" {
  description = "Members to grant bigtable.user role"
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
