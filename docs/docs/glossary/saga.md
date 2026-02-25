---
id: saga
title: Saga
hoverText: Stateless domain bridge translating events from one domain into commands for another.
---

# Saga

Domain bridge component. Events from one domain in, commands to another domain out. Stateless translation between bounded contexts.

**Abbreviation:** `sag`

Sagas should contain minimal logic - just enough to map fields and construct target commands. If you need conditionals or business rules, that logic belongs in the source aggregate.

Sagas must be stateless - each event is processed independently with no memory of previous events.
