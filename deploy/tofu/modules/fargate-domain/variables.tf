# Fargate Domain Module - Variables (Placeholder)

variable "domain" {
  description = "Domain name"
  type        = string
}

variable "cluster_arn" {
  description = "ECS cluster ARN"
  type        = string
}

variable "vpc_id" {
  description = "VPC ID"
  type        = string
}

variable "subnet_ids" {
  description = "Subnet IDs"
  type        = list(string)
}

variable "aggregate" {
  description = "Aggregate configuration"
  type = object({
    image = string
    env   = optional(map(string), {})
  })
}

variable "sagas" {
  description = "Saga configurations"
  type = map(object({
    target_domain = string
    image         = string
    env           = optional(map(string), {})
  }))
  default = {}
}

variable "projectors" {
  description = "Projector configurations"
  type = map(object({
    image = string
    env   = optional(map(string), {})
  }))
  default = {}
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
  description = "Labels (tags)"
  type        = map(string)
  default     = {}
}
