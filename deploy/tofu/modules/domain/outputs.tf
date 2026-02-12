# Domain Module - Outputs

output "component_url" {
  description = "URL of the main component (aggregate or process manager)"
  value = var.aggregate.enabled ? (
    length(google_cloud_run_v2_service.aggregate) > 0 ? google_cloud_run_v2_service.aggregate[0].uri : null
  ) : (
    var.process_manager.enabled && length(google_cloud_run_v2_service.process_manager) > 0 ? google_cloud_run_v2_service.process_manager[0].uri : null
  )
}

output "component_name" {
  description = "Name of the main component service"
  value = var.aggregate.enabled ? (
    length(google_cloud_run_v2_service.aggregate) > 0 ? google_cloud_run_v2_service.aggregate[0].name : null
  ) : (
    var.process_manager.enabled && length(google_cloud_run_v2_service.process_manager) > 0 ? google_cloud_run_v2_service.process_manager[0].name : null
  )
}

output "saga_urls" {
  description = "Map of saga name to service URL"
  value = {
    for name, service in google_cloud_run_v2_service.saga : name => service.uri
  }
}

output "projector_urls" {
  description = "Map of projector name to service URL"
  value = {
    for name, service in google_cloud_run_v2_service.projector : name => service.uri
  }
}

output "service_account" {
  description = "Service account email for this domain"
  value       = local.service_account
}

# Discovery entries for registry module
output "discovery_entries" {
  description = "All service URLs for registry aggregation"
  value = merge(
    # Main component (aggregate or PM)
    var.aggregate.enabled && length(google_cloud_run_v2_service.aggregate) > 0 ? {
      "ANGZARR_AGGREGATE_${upper(var.domain)}" = google_cloud_run_v2_service.aggregate[0].uri
    } : {},
    var.process_manager.enabled && length(google_cloud_run_v2_service.process_manager) > 0 ? {
      "ANGZARR_PM_${upper(var.domain)}" = google_cloud_run_v2_service.process_manager[0].uri
    } : {},
    # Sagas
    {
      for name, service in google_cloud_run_v2_service.saga :
      "ANGZARR_SAGA_${upper(var.domain)}_${upper(name)}" => service.uri
    },
    # Projectors
    {
      for name, service in google_cloud_run_v2_service.projector :
      "ANGZARR_PROJECTOR_${upper(var.domain)}_${upper(name)}" => service.uri
    }
  )
}

# Structured discovery for JSON format
output "discovery_json" {
  description = "Structured discovery data for JSON serialization"
  value = {
    domain = var.domain
    aggregate = var.aggregate.enabled && length(google_cloud_run_v2_service.aggregate) > 0 ? {
      url  = google_cloud_run_v2_service.aggregate[0].uri
      name = google_cloud_run_v2_service.aggregate[0].name
    } : null
    process_manager = var.process_manager.enabled && length(google_cloud_run_v2_service.process_manager) > 0 ? {
      url            = google_cloud_run_v2_service.process_manager[0].uri
      name           = google_cloud_run_v2_service.process_manager[0].name
      source_domains = var.process_manager.source_domains
    } : null
    sagas = {
      for name, service in google_cloud_run_v2_service.saga : name => {
        url           = service.uri
        name          = service.name
        target_domain = var.sagas[name].target_domain
      }
    }
    projectors = {
      for name, service in google_cloud_run_v2_service.projector : name => {
        url  = service.uri
        name = service.name
      }
    }
  }
}
