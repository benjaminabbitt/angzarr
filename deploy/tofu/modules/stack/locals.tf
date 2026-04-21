# Stack Module - Locals
# Computed values for validation and topology

locals {
  #----------------------------------------------------------------------------
  # Domain Analysis
  #----------------------------------------------------------------------------

  domain_names = toset(keys(var.domains))

  # All sagas with their source domain
  all_sagas = flatten([
    for domain_name, domain in var.domains : [
      for saga_name, saga in domain.sagas : {
        name          = saga_name
        source_domain = domain_name
        target_domain = saga.target_domain
        image         = saga.image
        env           = saga.env
      }
    ]
  ])

  # All projectors with their source domain
  all_projectors = flatten([
    for domain_name, domain in var.domains : [
      for proj_name, proj in domain.projectors : {
        name          = proj_name
        source_domain = domain_name
        image         = proj.image
        env           = proj.env
      }
    ]
  ])

  #----------------------------------------------------------------------------
  # Saga Target Validation
  #----------------------------------------------------------------------------

  saga_target_domains = toset([for s in local.all_sagas : s.target_domain])

  invalid_saga_targets = [
    for s in local.all_sagas : s.target_domain
    if !contains(local.domain_names, s.target_domain)
  ]

  #----------------------------------------------------------------------------
  # Process Manager Analysis
  #----------------------------------------------------------------------------

  pm_names = toset(keys(var.process_managers))

  pm_subscription_domains = toset(flatten([
    for pm_name, pm in var.process_managers : pm.subscriptions
  ]))

  pm_target_domains = toset(flatten([
    for pm_name, pm in var.process_managers : pm.targets
  ]))

  invalid_pm_subscriptions = [
    for d in local.pm_subscription_domains : d
    if !contains(local.domain_names, d)
  ]

  invalid_pm_targets = [
    for d in local.pm_target_domains : d
    if !contains(local.domain_names, d)
  ]

  #----------------------------------------------------------------------------
  # Interconnectedness Analysis
  #----------------------------------------------------------------------------

  # Domains that are saga sources (have outbound sagas)
  saga_sources = toset([for s in local.all_sagas : s.source_domain])

  # Domains that are saga targets (receive commands from sagas)
  saga_targets = toset([for s in local.all_sagas : s.target_domain])

  # Connected domains (source or target of any saga/PM)
  connected_domains = setunion(
    local.saga_sources,
    local.saga_targets,
    local.pm_subscription_domains,
    local.pm_target_domains
  )

  # Entry-point domains (explicitly marked, exempt from orphan validation)
  entry_point_domains = toset([
    for name, domain in var.domains : name
    if domain.entry_point
  ])

  # Orphan domains (not connected and not marked as entry point)
  orphan_domains = [
    for name in local.domain_names : name
    if !contains(local.connected_domains, name) && !contains(local.entry_point_domains, name)
  ]

  # Entry points: domains with no inbound sagas or PM commands
  inbound_domains = setunion(local.saga_targets, local.pm_target_domains)
  entry_points = [
    for name in local.domain_names : name
    if !contains(local.inbound_domains, name)
  ]

  #----------------------------------------------------------------------------
  # Storage Resolution (domain override > default)
  #----------------------------------------------------------------------------

  domain_storage = {
    for name, domain in var.domains : name => {
      event_store = coalesce(
        try(domain.storage.event_store, null),
        var.default_storage.event_store
      )
      position_store = coalesce(
        try(domain.storage.position_store, null),
        var.default_storage.position_store
      )
      snapshot_store = try(domain.storage.snapshot_store, null) != null ? (
        domain.storage.snapshot_store
      ) : var.default_storage.snapshot_store
    }
  }

  pm_storage = {
    for name, pm in var.process_managers : name => {
      event_store = coalesce(
        try(pm.storage.event_store, null),
        var.default_storage.event_store
      )
      position_store = coalesce(
        try(pm.storage.position_store, null),
        var.default_storage.position_store
      )
      snapshot_store = try(pm.storage.snapshot_store, null) != null ? (
        pm.storage.snapshot_store
      ) : var.default_storage.snapshot_store
    }
  }

  #----------------------------------------------------------------------------
  # Rust Features Aggregation
  #----------------------------------------------------------------------------

  all_rust_features = distinct(flatten(concat(
    # Bus features
    tolist(var.bus.provides.rust_features),
    # Default storage features
    tolist(var.default_storage.event_store.provides.rust_features),
    tolist(var.default_storage.position_store.provides.rust_features),
    var.default_storage.snapshot_store != null ? tolist(var.default_storage.snapshot_store.provides.rust_features) : [],
    # Domain storage overrides
    flatten([
      for name, storage in local.domain_storage : concat(
        tolist(storage.event_store.provides.rust_features),
        tolist(storage.position_store.provides.rust_features),
        storage.snapshot_store != null ? tolist(storage.snapshot_store.provides.rust_features) : []
      )
    ]),
    # PM storage overrides
    flatten([
      for name, storage in local.pm_storage : concat(
        tolist(storage.event_store.provides.rust_features),
        tolist(storage.position_store.provides.rust_features),
        storage.snapshot_store != null ? tolist(storage.snapshot_store.provides.rust_features) : []
      )
    ])
  )))

  #----------------------------------------------------------------------------
  # Labels
  #----------------------------------------------------------------------------

  common_labels = merge(
    {
      "angzarr-stack" = var.name
      "managed-by"    = "opentofu"
    },
    var.labels
  )
}
