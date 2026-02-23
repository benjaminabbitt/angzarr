# GitHub module variables

# =============================================================================
# Repository settings
# =============================================================================

variable "repository_name" {
  description = "Name of the GitHub repository"
  type        = string
}

variable "create_repository" {
  description = "Whether to create the repository or use existing"
  type        = bool
  default     = false
}

variable "description" {
  description = "Repository description"
  type        = string
  default     = ""
}

variable "visibility" {
  description = "Repository visibility: public, private, or internal"
  type        = string
  default     = "private"
  validation {
    condition     = contains(["public", "private", "internal"], var.visibility)
    error_message = "Visibility must be 'public', 'private', or 'internal'."
  }
}

variable "has_issues" {
  description = "Enable issues"
  type        = bool
  default     = true
}

variable "has_projects" {
  description = "Enable projects"
  type        = bool
  default     = false
}

variable "has_wiki" {
  description = "Enable wiki"
  type        = bool
  default     = false
}

variable "has_discussions" {
  description = "Enable discussions"
  type        = bool
  default     = false
}

variable "has_downloads" {
  description = "Enable downloads"
  type        = bool
  default     = false
}

variable "topics" {
  description = "Repository topics/tags"
  type        = list(string)
  default     = []
}

variable "archived" {
  description = "Archive the repository"
  type        = bool
  default     = false
}

variable "archive_on_destroy" {
  description = "Archive instead of delete on destroy"
  type        = bool
  default     = true
}

# =============================================================================
# Merge settings
# =============================================================================

variable "allow_merge_commit" {
  description = "Allow merge commits"
  type        = bool
  default     = false
}

variable "allow_squash_merge" {
  description = "Allow squash merging"
  type        = bool
  default     = true
}

variable "allow_rebase_merge" {
  description = "Allow rebase merging"
  type        = bool
  default     = false
}

variable "allow_auto_merge" {
  description = "Allow auto-merge on PRs"
  type        = bool
  default     = true
}

variable "delete_branch_on_merge" {
  description = "Delete head branch after merge"
  type        = bool
  default     = true
}

variable "squash_merge_commit_title" {
  description = "Title for squash merge commits: PR_TITLE or COMMIT_OR_PR_TITLE"
  type        = string
  default     = "PR_TITLE"
}

variable "squash_merge_commit_message" {
  description = "Message for squash merge commits: PR_BODY, COMMIT_MESSAGES, or BLANK"
  type        = string
  default     = "PR_BODY"
}

# =============================================================================
# Security
# =============================================================================

variable "vulnerability_alerts" {
  description = "Enable Dependabot vulnerability alerts"
  type        = bool
  default     = true
}

# =============================================================================
# Pages
# =============================================================================

variable "pages_enabled" {
  description = "Enable GitHub Pages"
  type        = bool
  default     = false
}

variable "pages_branch" {
  description = "Branch for GitHub Pages"
  type        = string
  default     = "gh-pages"
}

variable "pages_path" {
  description = "Path for GitHub Pages source"
  type        = string
  default     = "/"
}

variable "pages_cname" {
  description = "Custom domain for GitHub Pages"
  type        = string
  default     = null
}

# =============================================================================
# Branch protection
# =============================================================================

variable "branch_protection_rules" {
  description = "Branch protection rules"
  type = map(object({
    pattern                         = string
    enforce_admins                  = optional(bool, true)
    require_signed_commits          = optional(bool, false)
    required_linear_history         = optional(bool, true)
    require_conversation_resolution = optional(bool, true)
    allows_deletions                = optional(bool, false)
    allows_force_pushes             = optional(bool, false)
    lock_branch                     = optional(bool, false)

    required_status_checks = optional(object({
      strict   = optional(bool, true)
      contexts = optional(list(string), [])
    }), null)

    required_pull_request_reviews = optional(object({
      dismiss_stale_reviews           = optional(bool, true)
      restrict_dismissals             = optional(bool, false)
      dismissal_restrictions          = optional(list(string), [])
      pull_request_bypassers          = optional(list(string), [])
      require_code_owner_reviews      = optional(bool, false)
      required_approving_review_count = optional(number, 1)
      require_last_push_approval      = optional(bool, true)
    }), null)

    restrict_pushes = optional(object({
      blocks_creations = optional(bool, true)
      push_allowances  = optional(list(string), [])
    }), null)
  }))
  default = {}
}

# =============================================================================
# Actions secrets
# =============================================================================

variable "actions_secrets" {
  description = "Repository-level Actions secrets (name => value)"
  type        = map(string)
  default     = {}
  sensitive   = true
}

variable "actions_variables" {
  description = "Repository-level Actions variables (name => value)"
  type        = map(string)
  default     = {}
}

# =============================================================================
# Environments
# =============================================================================

variable "environments" {
  description = "Deployment environments configuration"
  type = map(object({
    wait_timer          = optional(number, 0)
    can_admins_bypass   = optional(bool, true)
    prevent_self_review = optional(bool, false)

    reviewers = optional(object({
      users = optional(list(number), [])
      teams = optional(list(number), [])
    }), null)

    deployment_branch_policy = optional(object({
      protected_branches     = optional(bool, true)
      custom_branch_policies = optional(bool, false)
    }), null)

    secrets   = optional(map(string), {})
    variables = optional(map(string), {})
  }))
  default = {}
}

# =============================================================================
# Webhooks
# =============================================================================

variable "webhooks" {
  description = "Repository webhooks"
  type = map(object({
    url          = string
    content_type = optional(string, "json")
    insecure_ssl = optional(bool, false)
    secret       = optional(string, null)
    active       = optional(bool, true)
    events       = list(string)
  }))
  default   = {}
  sensitive = true
}

# =============================================================================
# Actions permissions
# =============================================================================

variable "actions_access_level" {
  description = "Actions access level for the repository: none, user, organization, or enterprise"
  type        = string
  default     = "none"
  validation {
    condition     = contains(["none", "user", "organization", "enterprise"], var.actions_access_level)
    error_message = "Actions access level must be 'none', 'user', 'organization', or 'enterprise'."
  }
}

variable "allowed_actions" {
  description = "Which actions are allowed: all, local_only, or selected"
  type        = string
  default     = "all"
  validation {
    condition     = contains(["all", "local_only", "selected"], var.allowed_actions)
    error_message = "Allowed actions must be 'all', 'local_only', or 'selected'."
  }
}

variable "allowed_actions_patterns" {
  description = "Patterns for allowed actions when allowed_actions is 'selected'"
  type        = list(string)
  default     = []
}

variable "github_owned_allowed" {
  description = "Allow GitHub-owned actions when allowed_actions is 'selected'"
  type        = bool
  default     = true
}

variable "verified_allowed" {
  description = "Allow verified creator actions when allowed_actions is 'selected'"
  type        = bool
  default     = true
}
