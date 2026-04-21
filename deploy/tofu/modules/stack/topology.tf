# Stack Module - Topology
# Graph construction and mermaid output

locals {
  #----------------------------------------------------------------------------
  # Graph Nodes
  #----------------------------------------------------------------------------

  topology_nodes = concat(
    # Domain nodes
    [
      for name, domain in var.domains : {
        id          = name
        type        = "domain"
        entry_point = contains(local.entry_points, name) || domain.entry_point
      }
    ],
    # Process manager nodes
    [
      for name, pm in var.process_managers : {
        id          = name
        type        = "process_manager"
        entry_point = false
      }
    ]
  )

  #----------------------------------------------------------------------------
  # Graph Edges
  #----------------------------------------------------------------------------

  topology_edges = concat(
    # Saga edges (domain -> target_domain)
    [
      for s in local.all_sagas : {
        from = s.source_domain
        to   = s.target_domain
        via  = s.name
        type = "saga"
      }
    ],
    # PM subscription edges (domain -> PM)
    flatten([
      for pm_name, pm in var.process_managers : [
        for domain in pm.subscriptions : {
          from = domain
          to   = pm_name
          via  = "subscription"
          type = "pm_subscription"
        }
      ]
    ]),
    # PM command edges (PM -> domain)
    flatten([
      for pm_name, pm in var.process_managers : [
        for target in pm.targets : {
          from = pm_name
          to   = target
          via  = "command"
          type = "pm_command"
        }
      ]
    ])
  )

  #----------------------------------------------------------------------------
  # Mermaid Generation
  #----------------------------------------------------------------------------

  # Domain node lines
  mermaid_domain_nodes = [
    for name, _ in var.domains : "        ${name}[${name}]"
  ]

  # PM node lines (using double braces for hexagon shape)
  mermaid_pm_nodes = [
    for name, _ in var.process_managers : "        ${replace(name, "-", "_")}{{${name}}}"
  ]

  # Saga edge lines: source -->|saga-name| target
  mermaid_saga_edges = [
    for s in local.all_sagas :
    "    ${s.source_domain} -->|${s.name}| ${s.target_domain}"
  ]

  # PM subscription edge lines: domain -.->|subscribes| pm
  mermaid_pm_subscription_edges = flatten([
    for pm_name, pm in var.process_managers : [
      for domain in pm.subscriptions :
      "    ${domain} -.-> ${replace(pm_name, "-", "_")}"
    ]
  ])

  # PM command edge lines: pm ==>|commands| domain
  mermaid_pm_command_edges = flatten([
    for pm_name, pm in var.process_managers : [
      for target in pm.targets :
      "    ${replace(pm_name, "-", "_")} ==> ${target}"
    ]
  ])

  # Assemble mermaid diagram
  mermaid_lines = concat(
    ["flowchart LR"],
    ["    subgraph Domains"],
    local.mermaid_domain_nodes,
    ["    end"],
    length(var.process_managers) > 0 ? concat(
      [""],
      ["    subgraph Process_Managers[Process Managers]"],
      local.mermaid_pm_nodes,
      ["    end"]
    ) : [],
    [""],
    local.mermaid_saga_edges,
    length(var.process_managers) > 0 ? concat(
      [""],
      local.mermaid_pm_subscription_edges,
      local.mermaid_pm_command_edges
    ) : []
  )

  topology_mermaid = join("\n", local.mermaid_lines)
}
