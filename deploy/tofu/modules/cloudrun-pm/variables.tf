# Cloud Run PM Module - Variables (Placeholder)

variable "name" {
  description = "Process manager name"
  type        = string
}

variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
}

variable "image" {
  description = "Business logic image"
  type        = string
}

variable "subscriptions" {
  description = "Domains to subscribe to"
  type        = list(string)
}

variable "targets" {
  description = "Domains to emit commands to"
  type        = list(string)
}

variable "env" {
  description = "Environment variables"
  type        = map(string)
  default     = {}
}

variable "storage" {
  description = "Storage configuration"
  type        = any
}

variable "bus" {
  description = "Bus configuration"
  type        = any
}

variable "coordinator_images" {
  description = "Coordinator images"
  type        = any
}

variable "labels" {
  description = "Labels"
  type        = map(string)
  default     = {}
}
