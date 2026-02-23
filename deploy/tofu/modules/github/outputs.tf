# GitHub module outputs

output "repository_name" {
  description = "Repository name"
  value       = local.repository_name
}

output "repository_id" {
  description = "Repository node ID"
  value       = local.repository_id
}

output "repository_full_name" {
  description = "Full repository name (owner/repo)"
  value       = var.create_repository ? github_repository.this[0].full_name : data.github_repository.this[0].full_name
}

output "repository_html_url" {
  description = "Repository URL"
  value       = var.create_repository ? github_repository.this[0].html_url : data.github_repository.this[0].html_url
}

output "repository_ssh_clone_url" {
  description = "SSH clone URL"
  value       = var.create_repository ? github_repository.this[0].ssh_clone_url : data.github_repository.this[0].ssh_clone_url
}

output "repository_http_clone_url" {
  description = "HTTPS clone URL"
  value       = var.create_repository ? github_repository.this[0].http_clone_url : data.github_repository.this[0].http_clone_url
}

output "branch_protection_rules" {
  description = "Branch protection rule IDs"
  value = {
    for k, v in github_branch_protection.this : k => v.id
  }
}

output "environments" {
  description = "Environment names"
  value       = [for k, v in github_repository_environment.this : k]
}

output "webhook_ids" {
  description = "Webhook IDs"
  value = {
    for k, v in github_repository_webhook.this : k => v.id
  }
}
