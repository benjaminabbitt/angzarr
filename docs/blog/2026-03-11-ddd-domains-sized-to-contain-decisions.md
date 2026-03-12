---
slug: ddd-domains-sized-to-contain-decisions
title: "DDD: Domains Sized to Contain Decisions"
authors: [angzarr]
tags: [ddd, architecture, bounded-context, event-sourcing]
keywords: [domain-driven-design, bounded-context, aggregate, saga, event-sourcing, microservices, ddd-patterns]
---

**The uncomfortable truth: most DDD teams draw their bounded contexts too small.**

Not too large—too *small*. They slice by CRUD entity, by database table, by team org chart. The result? Contexts that cannot make decisions autonomously. Every meaningful operation requires cross-context coordination. The architecture devolves into a distributed monolith with extra network hops.

This post argues for a different principle: **a bounded context is correctly sized when every decision that changes its invariants can be made entirely within it, without synchronous runtime dependency on another context.**

<!-- truncate -->

:::note Terminology
In canonical DDD, "domain" is the problem space (e.g., "online poker"), while "bounded context" is the solution-space boundary containing multiple aggregates with shared language.

**Angzarr's "domain" is different.** An Angzarr domain is an aggregate namespace—one domain per aggregate type. The ownership model:

```
Team → Bounded Context (1:many, discouraged) → Domains (1:many) → Aggregate (1:1)
```

Teams *can* own multiple bounded contexts, but this is discouraged—language shifts between related contexts cause confusion. Each context owns one or more domains. Domain and bounded context remain different concepts—infrastructure boundary vs. organizational boundary. Teams track these mappings via K8s labels.

This post uses "bounded context" when discussing DDD theory. See the [glossary](/docs/glossary/domain) for the full mapping.
:::

## The Decision Containment Principle

Eric Evans defined a bounded context as having "a unified model—that is, internally consistent with no contradictions." He specified that teams should "explicitly define the context within which a model applies... keep the model strictly consistent within these bounds" <sup>[1](#ref-1)</sup>.

But what does "unified" and "consistent" mean in practice?

Here's the test: **Can this context enforce its own business rules without calling out?**

If the answer is "we need to ask the Orders context before we can validate a Payment," then either:
1. The Payment context is undersized, or
2. The concepts are in the wrong context entirely

This maps to Evans' idea that the model is the *decision-making unit*, not the data-holding unit. A context doesn't exist to hold data—it exists to make decisions about that data.

## Seven Principles for Decision-Containing Contexts

### 1. Invariant Ownership

Vaughn Vernon defines an invariant as "a business rule that must always be consistent, specifically referring to transactional consistency." He states: "A properly designed Aggregate is one that can be modified in any way required by the business with its invariants completely consistent within a single transaction" <sup>[3](#ref-3)</sup>.

The implication: **every business invariant must have exactly one context that owns and enforces it.** If two contexts share enforcement of the same rule, you have hidden coupling—a seam that will cause consistency bugs under load.<sup>[†](#derived-1)</sup>

**In Angzarr:** Each aggregate enforces its invariants via the `guard()` → `validate()` → `compute()` handler pattern. The aggregate receives commands, validates against current state, and emits events—all within a single transaction. Cross-aggregate coordination happens asynchronously via sagas, never synchronously within a command handler.

### 2. Ubiquitous Language as Boundary Signal

Fowler, interpreting Evans, notes: "Usually the dominant factor drawing boundaries between contexts is human culture—since models act as ubiquitous language, you need a different model when the language changes. Different groups of people will use subtly different vocabularies in different parts of a large organization" <sup>[1](#ref-1)</sup>.

The practical test:
- When two teams use the same word to mean different things → context boundary
- When one team explains concepts using another team's vocabulary → wrong context

Evans himself has clarified that "one confusion teams often have is differentiating between bounded contexts and subdomains. In an ideal world they coincide, but in reality they are often misaligned" <sup>[5](#ref-5)</sup>.

### 3. Autonomy Over Consistency

Vernon provides the architectural pattern: "There is a practical way to support eventual consistency in a DDD model. An Aggregate command method publishes a Domain Event that is in time delivered to one or more asynchronous subscribers. Each subscriber then retrieves a different yet corresponding Aggregate instance and executes its behavior based on it, each in a separate transaction" <sup>[3](#ref-3)</sup>.

Microsoft's architecture guidance reinforces this: "When a business process spans multiple aggregates, use domain events rather than a single transaction. Reference other aggregates by identity only—this decoupling maps directly to microservice boundaries" <sup>[4](#ref-4)</sup>.

**The principle: prefer eventual consistency across context boundaries over synchronous consistency.** If strong consistency is required between two aggregates at runtime, they probably belong in the same context—or your transaction boundary is wrong.

**In Angzarr:** Aggregates modify only themselves per transaction. Cross-domain communication flows through sagas (stateless translation) or process managers (stateful coordination). Both typically operate asynchronously on committed events.

Angzarr *does* support synchronous modes for cross-domain calls, but discourages their use—they reintroduce the coupling and availability problems eventual consistency solves. Use sync modes only when business requirements genuinely demand it and you've accepted the tradeoffs.

### 4. Aggregate as Unit of Transactional Consistency

Vernon is explicit: "The consistency boundary logically asserts that everything inside adheres to a specific set of business invariant rules no matter what operations are performed. The consistency of everything outside this boundary is irrelevant to the Aggregate. Aggregates are chiefly about consistency boundaries and not driven by a desire to design object graphs" <sup>[3](#ref-3)</sup>.

ArchiLab reinforces: "A properly designed Aggregate is one that can be modified in any way required by the business with its invariants completely consistent within a single transaction. The consequence of this is that in one transaction, you can only modify one aggregate and never more than one aggregate" <sup>[11](#ref-11)</sup>.

**The aggregate boundary is not the context boundary—but it's a lower bound.** A context should contain all aggregates whose invariants reference each other.

**In Angzarr:** Each domain maps to exactly one aggregate type. Each aggregate instance is identified by `{domain}:{root_id}`. Multiple Angzarr domains may belong to the same DDD bounded context—they share ubiquitous language and team ownership, but are separate deployment units connected by sagas.

If aggregates share invariants, they either belong in the same aggregate (larger boundary) or require explicit coordination via sagas. Angzarr makes this choice visible in infrastructure rather than hiding it in code organization.

### 5. Anti-Corruption Layer as a Smell at Scale

The Anti-Corruption Layer is the integration pattern where a downstream bounded context translates concepts from an upstream context, protecting its own model from the upstream's influence.

ACLs are correct and necessary at integration points. But if a context needs a *thick* ACL—translating many concepts—the boundary may warrant re-examination. Sometimes the downstream context is missing concepts it should own; sometimes the upstream context is leaking internal details; sometimes it's unavoidable legacy integration.<sup>[†](#derived-2)</sup>

**In Angzarr:** Sagas connect Angzarr domains, but not all sagas are ACLs. The distinction:

- **Internal coordination sagas**: Connect domains *within* the same bounded context. Shared ubiquitous language means minimal translation—mostly routing.
- **ACL sagas**: Cross bounded context boundaries. Different teams, different language. Some translation expected.

The *thickness* of translation is the signal:
- **Thin ACL** (mapping a few concepts): Normal and expected when crossing BC boundaries
- **Thick ACL** (translating many concepts, complex mappings): Smell—suggests the boundary is in the wrong place or concepts are in the wrong context

### 6. Commands Stay Local, Events Cross Boundaries

The pattern is clear in the literature: domain events stay within the bounded context; integration events are the public contracts for cross-context communication. Commands express intent, and aggregates enforce rules.

Microsoft's guidance distinguishes domain events (internal notifications) from integration events (cross-context asynchronous communication) <sup>[4](#ref-4)</sup>.

**A well-sized context accepts commands and enforces rules locally.** It publishes domain events for others to react to. If a context issues commands *into* another context to complete its own operation, the command's logic belongs in the first context.

**In Angzarr:** Aggregates accept commands and emit events. Sagas translate events from one domain into facts (or commands) for another. The default saga output is *facts*—events the receiving domain must accept.

Whether a saga is "translation" (ACL) or "routing" (internal coordination) depends on whether the domains share a bounded context. Angzarr doesn't enforce this—it's an organizational decision tracked via K8s labels:

```yaml
labels:
  angzarr.io/bounded-context: "game-ops"
  angzarr.io/saga-type: "acl"  # or "internal"
```

This makes the distinction queryable and enforceable via policy. ACLs crossing context boundaries justify heavy translation logic; internal sagas should be thin.

### 7. Conway's Law Alignment

Fowler states: "Domain-Driven Design plays a role with Conway's Law in helping define organization structures, since a key part of DDD is to identify Bounded Contexts. A key characteristic of a Bounded Context is that it has its own Ubiquitous Language, defined and understood by the group of people working in that context. The key thing to remember about Conway's Law is that the modular decomposition of a system and the decomposition of the development organization must be done together" <sup>[9](#ref-9)</sup>.

Steve Smith (Ardalis) reinforces this: teams and bounded contexts should correlate, since cross-team ownership of a context risks applying the wrong assumptions or model <sup>[10](#ref-10)</sup>.

Microsoft provides the operational guidance: "If a single team must own multiple unrelated bounded contexts, or a single bounded context requires coordination across many teams, revisit either the boundaries or the team structure" <sup>[4](#ref-4)</sup>.

**Domain boundaries should align with team ownership boundaries.** A context that spans two teams without a clear seam will degrade—the ubiquitous language will fork, and the model will develop inconsistencies that mirror org chart politics.

## Common Failure Modes

### The Anemic Context

A context that owns data but no decisions. All business logic lives in an application service that orchestrates across multiple contexts. Looks like a context, acts like a database table.

This is the context-level manifestation of the anemic domain model anti-pattern: domain objects that contain little or no business logic, serving primarily as data structures while business logic lives in separate service layers.<sup>[†](#derived-3)</sup>

### The God Context

One context is sized to "fully contain decisions" by absorbing everything. Correct principle, wrong solution.

Evans himself warned that "total unification of the domain model for a large system will not be feasible or cost-effective" <sup>[1](#ref-1)</sup>. The fix is decomposing by subdomain (core, supporting, generic) and finding the natural seams—not abandoning the decision containment principle.<sup>[†](#derived-4)</sup>

### The Leaky Aggregate

An aggregate that enforces invariants but references foreign IDs without local projections, so any validation requires an outbound call.

Vernon explicitly warns against this: "Large aggregates are an anti-pattern. A large-cluster Aggregate will never perform or scale well, and is more likely to fail because false invariants and compositional convenience drove the design, to the detriment of transactional success, performance, and scalability" <sup>[11](#ref-11)</sup>.

The aggregate boundary is wrong, not the context boundary.

**In Angzarr:** Aggregates may query *external*, non-event systems (third-party APIs, legacy databases) during command handling to gather decision-making information. But they should only *read*—never write.

The better pattern: external systems holding state relevant to an aggregate should *inject* that context as facts into the aggregate, rather than the aggregate pulling it. Push beats pull—the aggregate's state becomes self-contained, and you avoid synchronous dependencies during command handling.

If your aggregate needs data from another Angzarr domain to validate, that's a smell. Either project that data locally, adjust the aggregate boundary, or reconsider whether the decision belongs in a different aggregate.

### Premature Context Split

A single business capability is split into two contexts before the model is stable—typically because of team structure. The two halves immediately develop tight coupling because the model isn't ready to be separated.

Evans has warned against "the bandwagon effect of jumping into microservices and bounded context splits." He notes "a common misconception is that a microservice is a bounded context, which he calls an oversimplification. When subdomains and bounded contexts are misaligned—such as when a business reorganization creates new subdomains that don't match existing bounded contexts—this often results in two teams having to work in the same context with increasing risk of ending up with a big ball of mud" <sup>[5](#ref-5)</sup>.

**Practitioner wisdom: keep the model in one context longer than feels comfortable, until the language stabilizes.**<sup>[†](#derived-5)</sup> That "longer than comfortable" state? It may be your legacy system—many monoliths are exactly this, never split because the language never stabilized. That's not always wrong; sometimes the domain genuinely is one context.

## Metrics for Evaluating Domain Boundaries

Most of these require architecture review rather than automated measurement, but several can be approximated from code and incident data.

### Structural Metrics

| Metric | Healthy Signal | Warning Signal |
|--------|----------------|----------------|
| Cross-context synchronous calls per operation | Few | Many |
| Shared database tables between contexts | None | Any |
| Aggregate references to foreign-context IDs without local copy | Rare | Common—suggests incomplete model |
| ACL translation surface (# of concepts mapped) | Thin | Thick |
| Number of context owners per business capability | One | Multiple |

These are heuristics, not empirically-derived thresholds. "Few" vs "many" depends on your latency budget and availability requirements. The point is directional: more cross-context coupling = more boundary debt.

Bounded context sizing is a Goldilocks problem:
- **Too small**: Contexts can't make decisions alone, requiring constant cross-context coordination (the main thesis of this post)
- **Too large**: Contexts become unmaintainable, language diverges internally, teams step on each other (the "God Context" failure mode)
- **Just right**: Each context contains the decisions it needs to make, no more<sup>[†](#derived-6)</sup>

Microsoft's guidance: "Design aggregates to be no smaller than what is required to enforce an invariant within a single transaction. Include only the data that must remain consistent within a single transaction. When you combine unrelated aggregates, you force unrelated updates to compete for the same locks" <sup>[4](#ref-4)</sup>.

### Operational Metrics

| Metric | What It Reveals |
|--------|-----------------|
| Blast radius of a context failure | How many business capabilities fail when this context is unavailable—high blast radius suggests context is too large |
| Deployment coupling frequency | How often does deploying context A require coordinating with context B? Frequent coordination = implicit coupling |
| Cross-context incident correlation | When context A degrades, does context B degrade? Correlated failures suggest hidden coupling |
| Time to make a model change | Long time = concept is contested or shared across contexts |

These metrics align with DORA research and Team Topologies guidance on measuring team and system boundary alignment <sup>[13](#ref-13)</sup>.

### Decision Containment Score

For each key business decision the domain owns, ask:

1. Does making this decision require data from another context at runtime? (synchronous query = −1)
2. Does enforcing the resulting invariant require another context's cooperation? (−1)
3. Does rolling back a failed decision require coordinating with another context? (−1)

**A score of 0 across all decisions is the target.** Anything below −1 per decision indicates boundary misalignment.<sup>[†](#derived-7)</sup>

## Sizing Heuristics

**Start with subdomains, not microservices.** There are three types of subdomains: "Core, Supporting, and Generic. The Core subdomain is where the business must put its best efforts and provides competitive advantage. The Supporting subdomain complements the main domain. The Generic subdomain is typically handled by ready-made commercial or open-source software" <sup>[14](#ref-14)</sup>. Subdomain analysis gives you the strategic cuts first. Bounded contexts then follow subdomain contours.

**A context should be deployable and operable by one team.** Not one person, not five teams. "Architectural and team evolution must go hand-in-hand throughout the life of an enterprise" <sup>[9](#ref-9)</sup>.

**The model should fit in one person's head.** If explaining the context's model requires a two-hour meeting, it's too large.<sup>[†](#derived-8)</sup>

**Event volume is not a sizing signal.** High event throughput is a scaling concern, not a domain boundary concern.<sup>[†](#derived-9)</sup>

## Where the Literature May Overcorrect

The principles above represent mainstream DDD thinking. But having built [Angzarr](/docs/getting-started)—an event-sourcing framework for distributed systems—I've encountered cases where rigid adherence to these rules creates its own problems.

:::warning Flexibility Has Consequences
Angzarr aims to be fast, reliable, and *flexible*. That flexibility permits building terrible systems:

- **Synchronous cascades** across dozens of aggregates—causing performance problems and availability nightmares
- **Poor aggregate factoring**—undersized aggregates that can't make decisions alone cause explosions in cross-domain messages and degraded performance
- **Sagas emitting commands**—the mechanism for cross-aggregate decisions, but adds compensation complexity; overuse often signals poor aggregate factoring
- **God process managers**—PMs that orchestrate everything become a single point of failure and a coordination bottleneck; decision logic belongs in aggregates, not PMs
- **Ignoring every principle in this post**—Angzarr won't stop you

The thesis of this post applies to Angzarr itself: aggregates should make decisions with minimum external contact. Violate that, and you'll pay in latency, throughput, and operational complexity.

Sometimes these anti-patterns are necessary—even the *right* choice for your constraints. Angzarr supports them for that reason. But it takes no responsibility for the consequences. We warn you in documentation and, often, in code—make sure you're choosing the tradeoff deliberately, not accidentally.
:::

### The Refactoring Problem

Here's the uncomfortable truth the literature rarely addresses: **DDD boundaries are architecture, and architecture is expensive to change.**

Conway's Law cuts both ways. Yes, system structure should align with team structure. But once it does, that alignment becomes load-bearing. Refactoring a bounded context boundary likely means some combination of:
- Reorganizing teams (politics, HR, reporting structures)
- Migrating data between stores (downtime, consistency risks)
- Rewriting integration contracts (coordinated deployments)
- Updating monitoring, alerting, and runbooks (operational knowledge)

The literature says "if your context can't make decisions autonomously, it's undersized—fix it." That's correct in principle. But fixing it may require executive buy-in, a migration project, and months of coordination. Meanwhile, the business needs to ship features.

**Angzarr takes a pragmatic stance: support sub-ideal boundaries with tooling when refactoring isn't feasible.**

This isn't an endorsement of bad architecture. It's an acknowledgment that production systems exist, Conway's Law has inertia, and sometimes the operationally necessary choice is to work within existing constraints while planning longer-term improvements.

### The "Commands Stay Local" Oversimplification

The principle that commands should stay local while only events cross boundaries is elegant in theory. In practice, it can force awkward aggregate designs.

Consider a saga that translates an `OrderCompleted` event into fulfillment work. The fulfillment domain needs to create a shipment. Under strict "events only" thinking, the saga should publish an event like `FulfillmentRequested`, which the fulfillment context reacts to.

But what happens when fulfillment fails? The saga has no mechanism to compensate—it fired an event and walked away. The fulfillment context now owns the problem entirely, even though the *business process* spans both domains.

**Angzarr takes a different approach.** Sagas can emit either commands or *facts* to other aggregates:

**Facts** (the default for saga output): Events injected without a preceding command. Structurally, facts are just events with two differences: the *receiving* domain assigns the sequence number, and they retain source traceability metadata. The receiving aggregate cannot reject them; they represent external realities. Example: "the hand says it's your turn" is a fact the player aggregate must accept.

**Commands** (when compensation is needed): Requests that the receiving aggregate can reject. Use commands when:
1. The receiving aggregate should be able to refuse (insufficient inventory, invalid state)
2. Rejection must trigger compensation in the originating domain
3. The saga uses destination state to make business decisions

Both patterns require:
- The saga uses the destination aggregate's state to inform decisions
- Sequence validation ensures exactly-once delivery semantics

**General guidance: prefer facts for saga output unless you need rejection/compensation capability.** Facts are simpler—they represent "this happened" rather than "please do this." Commands add complexity but enable explicit failure handling.

**A warning:** If you find yourself reaching for saga-emitted commands frequently, pause and ask whether your bounded contexts are correctly sized. A saga that needs to send commands with compensation capability may be a signal that:
- The two aggregates belong in the same context (shared invariants requiring coordination)
- The decision logic is in the wrong aggregate (should move upstream)
- A process manager is more appropriate than a saga (Angzarr's process managers are stateful, use the correlation ID as their aggregate root, and explicitly coordinate multi-domain workflows)

Angzarr supports the pattern because sometimes it's genuinely correct. But "supported" doesn't mean "encouraged." Treat saga-emitted commands as a code smell worth investigating, even when it's the right solution.

This nuance isn't captured by the simple "commands stay local, events cross boundaries" rule. The question isn't command-vs-event—it's whether the receiving domain has veto power over the incoming information.

### Aggregate Size: The Overloading Risk

Vernon's guidance to keep aggregates small—containing only what's needed for invariant enforcement in a single transaction—is sound. But it can be taken too far.

An aggregate that's too small becomes a data container that delegates all decisions outward. Every validation requires a saga or process manager to orchestrate across aggregates. You've achieved small aggregates at the cost of coherent decision-making.

The opposite risk is real too: an aggregate that absorbs everything becomes a bottleneck. But in my experience, **teams more often err toward undersized aggregates** than oversized ones—particularly when influenced by microservices culture that conflates "small services" with good architecture.

The test isn't aggregate size. It's: **can this aggregate make its decisions without runtime dependencies?**

### A Pragmatic Middle Ground

Angzarr supports patterns the literature flags as anti-patterns—**but discourages them**:

| Pattern | Orthodox View | Angzarr's Position |
|---------|---------------|-------------------|
| Sagas emitting commands | Avoid—events only | Supported but discouraged; prefer [facts](/docs/features/facts) |
| Cascading synchronous calls | Never | Supported for legacy boundaries; refactor when possible |
| Undersized aggregates requiring coordination | Anti-pattern | Supported with [compensation](/docs/features/compensation) tooling; indicates boundary debt |
| "Large" aggregates | Anti-pattern | Sometimes correct—if the aggregate owns a cohesive set of decisions |

**These are escape hatches, not recommended patterns.** Each represents technical debt—a workaround for boundaries that should ideally be redrawn. Angzarr provides the tooling because:

1. **Production systems exist.** You inherited boundaries drawn by someone else, possibly years ago.
2. **Conway's Law has inertia.** Fixing the architecture may require fixing the org chart first.
3. **Business doesn't wait.** Features ship while migration projects are planned.

The correct response to needing these patterns is:
1. Use them to unblock the immediate work
2. Document the boundary debt
3. Plan the refactoring (even if it's quarters away)
4. Don't let "supported" become "normalized"

The literature provides excellent defaults. When you deviate, know *why* you're deviating and have a plan to stop.

The underlying principle remains: **size your contexts and aggregates to contain decisions.** When you can't—because the boundaries are already drawn and load-bearing—Angzarr helps you cope. But coping isn't thriving. Fix the boundaries when you can.

## The Underlying Principle

A domain boundary is a **decision boundary**, not a data boundary or a service boundary. Draw it where decisions are made, not where data lives.

Most teams err by slicing too thin—creating contexts that own data but cannot decide. The result is an architecture where every operation requires coordination, every deployment requires synchronization, and the system exhibits all the costs of distribution with none of the benefits of autonomy.

Size your contexts to contain decisions. If a context cannot enforce its invariants alone, it's too small.

---

## References

<span id="ref-1">**[1]**</span> Martin Fowler, "[BoundedContext](https://martinfowler.com/bliki/BoundedContext.html)," martinfowler.com (includes Evans quotations)

<span id="ref-3">**[3]**</span> Vaughn Vernon, "[Effective Aggregate Design](https://www.dddcommunity.org/library/vernon_2011/)," Parts I–III, dddcommunity.org (2011); also *Implementing Domain-Driven Design* (2013), Addison-Wesley

<span id="ref-4">**[4]**</span> Microsoft Azure Architecture Center, "[Design a DDD-oriented microservice](https://learn.microsoft.com/en-us/dotnet/architecture/microservices/microservice-ddd-cqrs-patterns/)," docs.microsoft.com

<span id="ref-5">**[5]**</span> Eric Evans at DDD Europe 2019, as covered by [InfoQ](https://www.infoq.com/news/2019/06/bounded-context-eric-evans/)

<span id="ref-9">**[9]**</span> Martin Fowler, "[Conway's Law](https://martinfowler.com/bliki/ConwaysLaw.html)," martinfowler.com

<span id="ref-10">**[10]**</span> Steve Smith (Ardalis), writings on bounded contexts and team organization

<span id="ref-11">**[11]**</span> ArchiLab, aggregate design based on Vernon's work

<span id="ref-13">**[13]**</span> Team Topologies literature on team/system boundary alignment

<span id="ref-14">**[14]**</span> DDD community resources on subdomain classification

---

## Notes on Derived Claims

The following claims in this post represent synthesis from the cited sources and practitioner experience, rather than direct quotations:

<span id="derived-1">**[†]**</span> The claim that "two contexts enforcing the same invariant produces hidden coupling" is extrapolated from Vernon's consistency boundary rules.

<span id="derived-2">**[†]**</span> The interpretation that thick ACLs warrant boundary re-examination is practitioner intuition, not a direct Evans/Vernon citation.

<span id="derived-3">**[†]**</span> The extension of the anemic domain model anti-pattern to the context level is derived analysis.

<span id="derived-4">**[†]**</span> "God Context" as a named failure mode is framing for this post, not canonical DDD terminology.

<span id="derived-5">**[†]**</span> Common practitioner advice without verified primary source.

<span id="derived-6">**[†]**</span> The structural metrics table is synthesized from the cited sources and practitioner literature, not a canonical table from Evans or Vernon.

<span id="derived-7">**[†]**</span> The Decision Containment Score is a framework constructed for this post as an operationalization of the decision containment principle. It does not appear in the primary sources.

<span id="derived-8">**[†]**</span> Common practitioner wisdom without verified primary source.

<span id="derived-9">**[†]**</span> "Event volume is not a sizing signal" is synthesis for this post without primary source support.
