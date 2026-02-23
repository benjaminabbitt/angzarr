# Branch protection rules

resource "github_branch_protection" "this" {
  for_each = var.branch_protection_rules

  repository_id = local.repository_id
  pattern       = each.value.pattern

  enforce_admins                  = each.value.enforce_admins
  require_signed_commits          = each.value.require_signed_commits
  required_linear_history         = each.value.required_linear_history
  require_conversation_resolution = each.value.require_conversation_resolution
  allows_deletions                = each.value.allows_deletions
  allows_force_pushes             = each.value.allows_force_pushes
  lock_branch                     = each.value.lock_branch

  # Required status checks
  dynamic "required_status_checks" {
    for_each = each.value.required_status_checks != null ? [each.value.required_status_checks] : []
    content {
      strict   = required_status_checks.value.strict
      contexts = required_status_checks.value.contexts
    }
  }

  # Required pull request reviews
  dynamic "required_pull_request_reviews" {
    for_each = each.value.required_pull_request_reviews != null ? [each.value.required_pull_request_reviews] : []
    content {
      dismiss_stale_reviews           = required_pull_request_reviews.value.dismiss_stale_reviews
      restrict_dismissals             = required_pull_request_reviews.value.restrict_dismissals
      dismissal_restrictions          = required_pull_request_reviews.value.dismissal_restrictions
      pull_request_bypassers          = required_pull_request_reviews.value.pull_request_bypassers
      require_code_owner_reviews      = required_pull_request_reviews.value.require_code_owner_reviews
      required_approving_review_count = required_pull_request_reviews.value.required_approving_review_count
      require_last_push_approval      = required_pull_request_reviews.value.require_last_push_approval
    }
  }

  # Push restrictions
  dynamic "restrict_pushes" {
    for_each = each.value.restrict_pushes != null ? [each.value.restrict_pushes] : []
    content {
      blocks_creations = restrict_pushes.value.blocks_creations
      push_allowances  = restrict_pushes.value.push_allowances
    }
  }
}
