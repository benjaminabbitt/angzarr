---
id: domain
title: Domain
hoverText: A bounded context representing a distinct business capability with cohesive aggregates.
---

# Domain

A bounded context representing a distinct business capability. Contains aggregates with cohesive behavior. Events/commands are namespaced by domain (e.g., `order`, `inventory`, `fulfillment`).

Each domain owns its data and logic - cross-domain communication happens only via events and commands through sagas.
