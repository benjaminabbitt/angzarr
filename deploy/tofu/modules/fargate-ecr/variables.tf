# Fargate ECR Module - Variables
# Creates ECR repositories for angzarr container images

variable "name_prefix" {
  description = "Prefix for ECR repository names"
  type        = string
  default     = "angzarr"
}

variable "image_tag_mutability" {
  description = "Tag mutability setting (MUTABLE or IMMUTABLE)"
  type        = string
  default     = "MUTABLE"
}

variable "scan_on_push" {
  description = "Enable image scanning on push"
  type        = bool
  default     = true
}

variable "encryption_type" {
  description = "Encryption type (AES256 or KMS)"
  type        = string
  default     = "AES256"
}

variable "kms_key_arn" {
  description = "KMS key ARN for encryption (if encryption_type is KMS)"
  type        = string
  default     = null
}

variable "lifecycle_policy_days" {
  description = "Number of days to keep untagged images"
  type        = number
  default     = 30
}

variable "tags" {
  description = "Tags to apply to resources"
  type        = map(string)
  default     = {}
}
