# Stack Module - Outputs

#------------------------------------------------------------------------------
# Topology
#------------------------------------------------------------------------------

output "topology_graph" {
  description = "Structured topology graph with nodes and edges"
  value = {
    nodes = local.topology_nodes
    edges = local.topology_edges
  }
}

output "topology_mermaid" {
  description = "Mermaid flowchart diagram of the topology"
  value       = local.topology_mermaid
}

output "entry_points" {
  description = "Domains that accept external commands (no inbound sagas/PM commands)"
  value       = local.entry_points
}

#------------------------------------------------------------------------------
# Rust Features
#------------------------------------------------------------------------------

output "rust_features" {
  description = "Aggregated Rust features required for this stack"
  value       = local.all_rust_features
}

#------------------------------------------------------------------------------
# Domain Information
#------------------------------------------------------------------------------

output "domains" {
  description = "Domain configuration details"
  value = {
    for name, domain in var.domains : name => {
      storage    = local.domain_storage[name]
      sagas      = keys(domain.sagas)
      projectors = keys(domain.projectors)
    }
  }
}

output "process_managers" {
  description = "Process manager configuration details"
  value = {
    for name, pm in var.process_managers : name => {
      storage       = local.pm_storage[name]
      subscriptions = pm.subscriptions
      targets       = pm.targets
    }
  }
}

#------------------------------------------------------------------------------
# Component URLs (populated after deployment)
#------------------------------------------------------------------------------

output "aggregate_urls" {
  description = "URLs for aggregate services (populated by platform module)"
  value = {
    for name, _ in var.domains : name => try(
      module.k8s_domain[name].aggregate_url,
      module.cloudrun_domain[name].aggregate_url,
      module.fargate_domain[name].aggregate_url,
      null
    )
  }
}

output "saga_urls" {
  description = "URLs for saga services (populated by platform module)"
  value = {
    for s in local.all_sagas : "${s.source_domain}-${s.name}" => try(
      module.k8s_domain[s.source_domain].saga_urls[s.name],
      module.cloudrun_domain[s.source_domain].saga_urls[s.name],
      module.fargate_domain[s.source_domain].saga_urls[s.name],
      null
    )
  }
}

output "pm_urls" {
  description = "URLs for process manager services (populated by platform module)"
  value = {
    for name, _ in var.process_managers : name => try(
      module.k8s_pm[name].url,
      module.cloudrun_pm[name].url,
      module.fargate_pm[name].url,
      null
    )
  }
}
