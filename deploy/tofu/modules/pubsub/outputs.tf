# Pub/Sub Module - Outputs

output "events_topic_id" {
  description = "ID of the main events topic"
  value       = google_pubsub_topic.events.id
}

output "events_topic_name" {
  description = "Name of the main events topic"
  value       = google_pubsub_topic.events.name
}

output "domain_topics" {
  description = "Map of domain names to topic IDs"
  value = {
    for domain, topic in google_pubsub_topic.domain : domain => topic.id
  }
}

output "dead_letter_topic_id" {
  description = "ID of the dead letter topic"
  value       = var.enable_dead_letter ? google_pubsub_topic.dead_letter[0].id : null
}

output "push_subscriptions" {
  description = "Map of subscriber names to subscription IDs"
  value = {
    for name, sub in google_pubsub_subscription.push : name => sub.id
  }
}

output "pull_subscription_id" {
  description = "ID of the pull subscription (if created)"
  value       = var.create_pull_subscription ? google_pubsub_subscription.pull[0].id : null
}

# Environment variables for angzarr configuration
output "coordinator_env" {
  description = "Environment variables for coordinator configuration"
  value = {
    PUBSUB_PROJECT_ID = var.project_id
    PUBSUB_TOPIC      = google_pubsub_topic.events.name
    BUS_TYPE          = "pubsub"
  }
}
