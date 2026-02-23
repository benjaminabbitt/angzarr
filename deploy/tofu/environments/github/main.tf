# GitHub repository configuration for angzarr
#
# This configures the angzarr-io/angzarr repository settings,
# branch protection, Actions secrets, and webhooks.
#
# Usage:
#   cd deploy/tofu/environments/github
#   tofu init
#   tofu plan -var-file=terraform.tfvars
#   tofu apply -var-file=terraform.tfvars
#
# Required environment variable:
#   GITHUB_TOKEN - Personal access token or GitHub App token with repo admin permissions

terraform {
  required_version = ">= 1.6.0"

  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
  }
}

provider "github" {
  owner = var.github_owner
  # Token from GITHUB_TOKEN environment variable
}

module "repository" {
  source = "../../modules/github"

  repository_name   = var.repository_name
  create_repository = false # Repository already exists

  # Merge settings - squash only, clean up branches
  allow_merge_commit     = false
  allow_squash_merge     = true
  allow_rebase_merge     = false
  allow_auto_merge       = true
  delete_branch_on_merge = true

  squash_merge_commit_title   = "PR_TITLE"
  squash_merge_commit_message = "PR_BODY"

  # Security
  vulnerability_alerts = true

  # Branch protection
  branch_protection_rules = {
    main = {
      pattern                         = "main"
      enforce_admins                  = true
      require_signed_commits          = false
      required_linear_history         = true
      require_conversation_resolution = true
      allows_deletions                = false
      allows_force_pushes             = false

      required_status_checks = {
        strict = true
        contexts = [
          "build-base",
          "build-rust",
          "fmt",
          "clippy",
          "test-unit",
          "test-integration",
          "test-unit-backends (postgres, storage_postgres, postgres)",
          "test-unit-backends (redis, storage_redis, redis)",
          "test-unit-backends (nats, bus_nats, nats)",
          "test-interface-bus (channel, channel, sqlite,channel)",
          "test-interface-bus (nats, nats, sqlite,nats)",
          "test-interface-bus (amqp, amqp, sqlite,amqp)",
          "test-interface-bus (kafka, kafka, sqlite,kafka)",
          "test-interface-bus (pubsub, pubsub, sqlite,pubsub)",
          "test-interface-bus (sns-sqs, sns-sqs, sqlite,sns-sqs)",
        ]
      }

      required_pull_request_reviews = {
        dismiss_stale_reviews           = true
        require_code_owner_reviews      = false
        required_approving_review_count = 1
        require_last_push_approval      = true
      }
    }
  }

  # Actions secrets (sensitive values from variables)
  actions_secrets = var.actions_secrets

  # Actions variables (non-sensitive)
  actions_variables = {
    REGISTRY = "ghcr.io"
  }

  # Deployment environments
  environments = {
    staging = {
      wait_timer        = 0
      can_admins_bypass = true

      deployment_branch_policy = {
        protected_branches     = true
        custom_branch_policies = false
      }

      secrets   = var.staging_secrets
      variables = var.staging_variables
    }

    production = {
      wait_timer          = 300 # 5 minute wait
      can_admins_bypass   = false
      prevent_self_review = true

      deployment_branch_policy = {
        protected_branches     = true
        custom_branch_policies = false
      }

      secrets   = var.production_secrets
      variables = var.production_variables
    }
  }

  # Webhooks
  webhooks = var.webhooks
}
