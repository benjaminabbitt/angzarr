# Pub/Sub Module
# Creates Google Cloud Pub/Sub topics and subscriptions for angzarr event bus
#
# Creates:
# - Main events topic (all domain events)
# - Per-domain topics (optional, for filtering)
# - Push subscriptions for sagas/projectors (delivers to Cloud Run)
# - Pull subscriptions (optional, for debugging/replay)

terraform {
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
  }
}

locals {
  labels = merge(
    {
      "managed-by" = "opentofu"
      "component"  = "event-bus"
    },
    var.labels
  )
}

# Main events topic - all domain events published here
resource "google_pubsub_topic" "events" {
  name    = var.events_topic_name
  project = var.project_id
  labels  = local.labels

  # Message retention for replay
  message_retention_duration = var.message_retention_duration

  # Schema enforcement (optional)
  dynamic "schema_settings" {
    for_each = var.schema_id != null ? [1] : []
    content {
      schema   = var.schema_id
      encoding = "JSON"
    }
  }
}

# Per-domain topics (optional - for domain-level filtering)
resource "google_pubsub_topic" "domain" {
  for_each = var.create_domain_topics ? toset(var.domains) : toset([])

  name    = "${var.events_topic_name}-${each.value}"
  project = var.project_id
  labels = merge(local.labels, {
    "angzarr-domain" = each.value
  })

  message_retention_duration = var.message_retention_duration
}

# Dead letter topic for failed messages
resource "google_pubsub_topic" "dead_letter" {
  count = var.enable_dead_letter ? 1 : 0

  name    = "${var.events_topic_name}-dead-letter"
  project = var.project_id
  labels  = local.labels

  message_retention_duration = "604800s" # 7 days
}

# Dead letter subscription (for monitoring/debugging)
resource "google_pubsub_subscription" "dead_letter" {
  count = var.enable_dead_letter ? 1 : 0

  name    = "${var.events_topic_name}-dead-letter-sub"
  topic   = google_pubsub_topic.dead_letter[0].name
  project = var.project_id
  labels  = local.labels

  # Keep messages for 7 days
  message_retention_duration = "604800s"
  retain_acked_messages      = true

  # Never expire
  expiration_policy {
    ttl = ""
  }
}

# Push subscriptions for sagas/projectors
resource "google_pubsub_subscription" "push" {
  for_each = var.push_subscribers

  name    = "${var.events_topic_name}-${each.key}"
  topic   = google_pubsub_topic.events.name
  project = var.project_id
  labels = merge(local.labels, {
    "subscriber" = each.key
    "subscriber-type" = lookup(each.value, "type", "saga")
  })

  # Push configuration - delivers to Cloud Run
  push_config {
    push_endpoint = each.value.endpoint

    # OIDC authentication for Cloud Run
    dynamic "oidc_token" {
      for_each = each.value.service_account != null ? [1] : []
      content {
        service_account_email = each.value.service_account
        audience              = each.value.endpoint
      }
    }

    # Attributes to include in push request
    attributes = {
      x-goog-version = "v1"
    }
  }

  # Ack deadline
  ack_deadline_seconds = var.ack_deadline_seconds

  # Retry policy
  retry_policy {
    minimum_backoff = var.retry_minimum_backoff
    maximum_backoff = var.retry_maximum_backoff
  }

  # Dead letter policy
  dynamic "dead_letter_policy" {
    for_each = var.enable_dead_letter ? [1] : []
    content {
      dead_letter_topic     = google_pubsub_topic.dead_letter[0].id
      max_delivery_attempts = var.max_delivery_attempts
    }
  }

  # Filter by domain (optional)
  dynamic "filter" {
    for_each = lookup(each.value, "domain_filter", null) != null ? [1] : []
    content {
      # Filter syntax: attributes.domain = "order"
      # See: https://cloud.google.com/pubsub/docs/filtering
      filter = "attributes.domain = \"${each.value.domain_filter}\""
    }
  }

  # Never expire
  expiration_policy {
    ttl = ""
  }
}

# Pull subscription for debugging/replay (optional)
resource "google_pubsub_subscription" "pull" {
  count = var.create_pull_subscription ? 1 : 0

  name    = "${var.events_topic_name}-pull"
  topic   = google_pubsub_topic.events.name
  project = var.project_id
  labels  = local.labels

  # Keep messages longer for replay scenarios
  message_retention_duration = "604800s" # 7 days
  retain_acked_messages      = true

  ack_deadline_seconds = 60

  # Never expire
  expiration_policy {
    ttl = ""
  }
}

# IAM: Allow aggregates to publish to events topic
resource "google_pubsub_topic_iam_member" "publishers" {
  for_each = toset(var.publishers)

  project = var.project_id
  topic   = google_pubsub_topic.events.name
  role    = "roles/pubsub.publisher"
  member  = each.value
}

# IAM: Allow Pub/Sub service account to push to Cloud Run
resource "google_project_iam_member" "pubsub_invoker" {
  for_each = var.grant_cloudrun_invoker ? toset(var.push_subscriber_accounts) : toset([])

  project = var.project_id
  role    = "roles/run.invoker"
  member  = each.value
}
