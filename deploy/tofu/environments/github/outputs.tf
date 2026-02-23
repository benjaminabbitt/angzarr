# GitHub environment outputs

output "repository_url" {
  description = "Repository URL"
  value       = module.repository.repository_html_url
}

output "branch_protection_rules" {
  description = "Branch protection rules configured"
  value       = module.repository.branch_protection_rules
}

output "environments" {
  description = "Deployment environments"
  value       = module.repository.environments
}
