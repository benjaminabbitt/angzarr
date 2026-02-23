# GitHub repository configuration module
#
# Manages repository settings, branch protection, actions, and webhooks.
# Requires a GitHub token with appropriate permissions.

terraform {
  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }
}

# Repository settings
resource "github_repository" "this" {
  count = var.create_repository ? 1 : 0

  name        = var.repository_name
  description = var.description
  visibility  = var.visibility

  # Features
  has_issues      = var.has_issues
  has_projects    = var.has_projects
  has_wiki        = var.has_wiki
  has_discussions = var.has_discussions
  has_downloads   = var.has_downloads

  # Pull request settings
  allow_merge_commit     = var.allow_merge_commit
  allow_squash_merge     = var.allow_squash_merge
  allow_rebase_merge     = var.allow_rebase_merge
  allow_auto_merge       = var.allow_auto_merge
  delete_branch_on_merge = var.delete_branch_on_merge

  # Squash merge commit settings
  squash_merge_commit_title   = var.squash_merge_commit_title
  squash_merge_commit_message = var.squash_merge_commit_message

  # Security
  vulnerability_alerts = var.vulnerability_alerts

  # Pages (if enabled)
  dynamic "pages" {
    for_each = var.pages_enabled ? [1] : []
    content {
      source {
        branch = var.pages_branch
        path   = var.pages_path
      }
      cname = var.pages_cname
    }
  }

  # Topics/tags
  topics = var.topics

  # Archive settings
  archived           = var.archived
  archive_on_destroy = var.archive_on_destroy

  lifecycle {
    prevent_destroy = true
  }
}

# Use data source when not creating repository
data "github_repository" "this" {
  count = var.create_repository ? 0 : 1
  name  = var.repository_name
}

locals {
  repository_name = var.create_repository ? github_repository.this[0].name : data.github_repository.this[0].name
  repository_id   = var.create_repository ? github_repository.this[0].node_id : data.github_repository.this[0].node_id
}
