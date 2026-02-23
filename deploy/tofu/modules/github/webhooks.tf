# Repository webhooks

# Use nonsensitive keys for iteration - webhook configs may contain secrets
locals {
  webhook_keys = nonsensitive(toset(keys(var.webhooks)))
}

resource "github_repository_webhook" "this" {
  for_each = local.webhook_keys

  repository = local.repository_name
  active     = var.webhooks[each.key].active
  events     = var.webhooks[each.key].events

  configuration {
    url          = var.webhooks[each.key].url
    content_type = var.webhooks[each.key].content_type
    insecure_ssl = var.webhooks[each.key].insecure_ssl
    secret       = var.webhooks[each.key].secret
  }
}
