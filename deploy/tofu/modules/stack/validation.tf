# Stack Module - Validation
# All check blocks run automatically during `tofu plan`

#------------------------------------------------------------------------------
# Saga Validation
#------------------------------------------------------------------------------

check "saga_targets_exist" {
  assert {
    condition     = length(local.invalid_saga_targets) == 0
    error_message = "Sagas reference non-existent domains: ${join(", ", local.invalid_saga_targets)}"
  }
}

#------------------------------------------------------------------------------
# Process Manager Validation
#------------------------------------------------------------------------------

check "pm_subscriptions_valid" {
  assert {
    condition     = length(local.invalid_pm_subscriptions) == 0
    error_message = "Process managers subscribe to non-existent domains: ${join(", ", local.invalid_pm_subscriptions)}"
  }
}

check "pm_targets_valid" {
  assert {
    condition     = length(local.invalid_pm_targets) == 0
    error_message = "Process managers target non-existent domains: ${join(", ", local.invalid_pm_targets)}"
  }
}

check "pm_has_subscriptions" {
  assert {
    condition = alltrue([
      for name, pm in var.process_managers : length(pm.subscriptions) > 0
    ])
    error_message = "All process managers must have at least one subscription"
  }
}

check "pm_has_targets" {
  assert {
    condition = alltrue([
      for name, pm in var.process_managers : length(pm.targets) > 0
    ])
    error_message = "All process managers must have at least one target"
  }
}

#------------------------------------------------------------------------------
# Topology Validation
#------------------------------------------------------------------------------

check "no_orphan_domains" {
  assert {
    condition     = length(local.orphan_domains) == 0
    error_message = "Orphan domains found (not connected via saga/PM and not marked standalone): ${join(", ", local.orphan_domains)}. Either connect them via sagas/PMs or mark as standalone = true."
  }
}

check "has_entry_points" {
  assert {
    condition     = length(local.entry_points) > 0
    error_message = "No entry points found. At least one domain must accept external commands (no inbound sagas/PM commands targeting it)."
  }
}

#------------------------------------------------------------------------------
# Name Collision Validation
#------------------------------------------------------------------------------

check "no_domain_pm_name_collision" {
  assert {
    condition     = length(setintersection(local.domain_names, local.pm_names)) == 0
    error_message = "Domain and process manager names must be unique. Collision: ${join(", ", setintersection(local.domain_names, local.pm_names))}"
  }
}

#------------------------------------------------------------------------------
# Storage Validation
#------------------------------------------------------------------------------

check "event_store_capability" {
  assert {
    condition     = contains(var.default_storage.event_store.provides.capabilities, "event_store")
    error_message = "default_storage.event_store must provide 'event_store' capability"
  }
}

check "position_store_capability" {
  assert {
    condition     = contains(var.default_storage.position_store.provides.capabilities, "position_store")
    error_message = "default_storage.position_store must provide 'position_store' capability"
  }
}

check "snapshot_store_capability" {
  assert {
    condition = (
      var.default_storage.snapshot_store == null ||
      try(contains(var.default_storage.snapshot_store.provides.capabilities, "snapshot_store"), false)
    )
    error_message = "default_storage.snapshot_store must provide 'snapshot_store' capability (or be null)"
  }
}
