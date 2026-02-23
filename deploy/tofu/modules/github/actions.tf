# GitHub Actions configuration: secrets, variables, environments

# =============================================================================
# Repository-level secrets
# =============================================================================

# Use nonsensitive keys for iteration - values remain sensitive
locals {
  actions_secret_keys = nonsensitive(toset(keys(var.actions_secrets)))
}

resource "github_actions_secret" "this" {
  for_each = local.actions_secret_keys

  repository      = local.repository_name
  secret_name     = each.key
  plaintext_value = var.actions_secrets[each.key]
}

# =============================================================================
# Repository-level variables
# =============================================================================

resource "github_actions_variable" "this" {
  for_each = var.actions_variables

  repository    = local.repository_name
  variable_name = each.key
  value         = each.value
}

# =============================================================================
# Environments
# =============================================================================

resource "github_repository_environment" "this" {
  for_each = var.environments

  repository  = local.repository_name
  environment = each.key

  wait_timer          = each.value.wait_timer
  can_admins_bypass   = each.value.can_admins_bypass
  prevent_self_review = each.value.prevent_self_review

  dynamic "reviewers" {
    for_each = each.value.reviewers != null ? [each.value.reviewers] : []
    content {
      users = reviewers.value.users
      teams = reviewers.value.teams
    }
  }

  dynamic "deployment_branch_policy" {
    for_each = each.value.deployment_branch_policy != null ? [each.value.deployment_branch_policy] : []
    content {
      protected_branches     = deployment_branch_policy.value.protected_branches
      custom_branch_policies = deployment_branch_policy.value.custom_branch_policies
    }
  }
}

# =============================================================================
# Environment secrets
# =============================================================================

resource "github_actions_environment_secret" "this" {
  for_each = merge([
    for env_name, env in var.environments : {
      for secret_name, secret_value in env.secrets :
      "${env_name}/${secret_name}" => {
        environment = env_name
        name        = secret_name
        value       = secret_value
      }
    }
  ]...)

  repository      = local.repository_name
  environment     = github_repository_environment.this[each.value.environment].environment
  secret_name     = each.value.name
  plaintext_value = each.value.value

  depends_on = [github_repository_environment.this]
}

# =============================================================================
# Environment variables
# =============================================================================

resource "github_actions_environment_variable" "this" {
  for_each = merge([
    for env_name, env in var.environments : {
      for var_name, var_value in env.variables :
      "${env_name}/${var_name}" => {
        environment = env_name
        name        = var_name
        value       = var_value
      }
    }
  ]...)

  repository    = local.repository_name
  environment   = github_repository_environment.this[each.value.environment].environment
  variable_name = each.value.name
  value         = each.value.value

  depends_on = [github_repository_environment.this]
}

# =============================================================================
# Actions permissions
# =============================================================================

resource "github_actions_repository_permissions" "this" {
  count = var.allowed_actions != "all" ? 1 : 0

  repository      = local.repository_name
  allowed_actions = var.allowed_actions

  dynamic "allowed_actions_config" {
    for_each = var.allowed_actions == "selected" ? [1] : []
    content {
      github_owned_allowed = var.github_owned_allowed
      verified_allowed     = var.verified_allowed
      patterns_allowed     = var.allowed_actions_patterns
    }
  }
}
