# SQLite Module - Variables

variable "data_dir" {
  description = <<-EOT
    Directory path for SQLite database files.

    For Kubernetes: Use a PersistentVolumeClaim mount path
    For local dev: Use a local directory path

    Each component will create its own database file in this directory:
    - {data_dir}/events.db
    - {data_dir}/positions.db
    - {data_dir}/snapshots.db
  EOT
  type    = string
  default = "/data/angzarr"
}

variable "journal_mode" {
  description = <<-EOT
    SQLite journal mode.

    - WAL (recommended): Write-Ahead Logging, better concurrency
    - DELETE: Traditional rollback journal
    - MEMORY: Journal in memory (faster but less durable)
  EOT
  type    = string
  default = "WAL"

  validation {
    condition     = contains(["WAL", "DELETE", "MEMORY", "TRUNCATE", "PERSIST", "OFF"], var.journal_mode)
    error_message = "journal_mode must be one of: WAL, DELETE, MEMORY, TRUNCATE, PERSIST, OFF"
  }
}

variable "synchronous" {
  description = <<-EOT
    SQLite synchronous mode.

    - FULL: Maximum durability, slower
    - NORMAL: Good balance (recommended for WAL mode)
    - OFF: Fastest, risk of corruption on crash
  EOT
  type    = string
  default = "NORMAL"

  validation {
    condition     = contains(["OFF", "NORMAL", "FULL", "EXTRA"], var.synchronous)
    error_message = "synchronous must be one of: OFF, NORMAL, FULL, EXTRA"
  }
}

variable "cache_size_kb" {
  description = "SQLite cache size in KB (negative = KB, positive = pages)"
  type        = number
  default     = -64000 # 64MB
}
