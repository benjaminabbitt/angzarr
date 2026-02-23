# Infrastructure Module: NATS - Variables

variable "name" {
  description = "Release name for NATS"
  type        = string
  default     = "angzarr-mq"
}

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "chart_version" {
  description = "NATS Helm chart version"
  type        = string
  default     = "1.1.0"
}

variable "jetstream_enabled" {
  description = "Enable JetStream for persistence"
  type        = bool
  default     = true
}

variable "jetstream_mem_size" {
  description = "JetStream memory storage size"
  type        = string
  default     = "256Mi"
}

variable "jetstream_file_enabled" {
  description = "Enable JetStream file storage"
  type        = bool
  default     = true
}

variable "jetstream_file_size" {
  description = "JetStream file storage size"
  type        = string
  default     = "1Gi"
}

variable "cluster_enabled" {
  description = "Enable NATS clustering"
  type        = bool
  default     = false
}

variable "replicas" {
  description = "Number of NATS replicas (if clustering enabled)"
  type        = number
  default     = 3
}

variable "nats_box_enabled" {
  description = "Enable NATS Box for debugging"
  type        = bool
  default     = false
}

variable "storage_class" {
  description = "Storage class for JetStream PVC"
  type        = string
  default     = ""
}
