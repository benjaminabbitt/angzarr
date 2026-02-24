---
title: Protocol Buffer API
description: Auto-generated documentation for Angzarr protobuf definitions
---

# Protocol Documentation
<a name="top"></a>

## Table of Contents

- [angzarr/aggregate.proto](#angzarr_aggregate-proto)
    - [BusinessResponse](#angzarr-BusinessResponse)
    - [CommandResponse](#angzarr-CommandResponse)
    - [ReplayRequest](#angzarr-ReplayRequest)
    - [ReplayResponse](#angzarr-ReplayResponse)
    - [RevocationResponse](#angzarr-RevocationResponse)
    - [SpeculateAggregateRequest](#angzarr-SpeculateAggregateRequest)
  
    - [AggregateCoordinatorService](#angzarr-AggregateCoordinatorService)
    - [AggregateService](#angzarr-AggregateService)
  
- [angzarr/cloudevents.proto](#angzarr_cloudevents-proto)
    - [CloudEvent](#angzarr-CloudEvent)
    - [CloudEvent.ExtensionsEntry](#angzarr-CloudEvent-ExtensionsEntry)
    - [CloudEventsResponse](#angzarr-CloudEventsResponse)
  
- [angzarr/meta.proto](#angzarr_meta-proto)
    - [DeleteEditionEvents](#angzarr-DeleteEditionEvents)
    - [EditionEventsDeleted](#angzarr-EditionEventsDeleted)
  
- [angzarr/process_manager.proto](#angzarr_process_manager-proto)
    - [ProcessManagerHandleRequest](#angzarr-ProcessManagerHandleRequest)
    - [ProcessManagerHandleResponse](#angzarr-ProcessManagerHandleResponse)
    - [ProcessManagerPrepareRequest](#angzarr-ProcessManagerPrepareRequest)
    - [ProcessManagerPrepareResponse](#angzarr-ProcessManagerPrepareResponse)
    - [SpeculatePmRequest](#angzarr-SpeculatePmRequest)
  
    - [ProcessManagerCoordinatorService](#angzarr-ProcessManagerCoordinatorService)
    - [ProcessManagerService](#angzarr-ProcessManagerService)
  
- [angzarr/projector.proto](#angzarr_projector-proto)
    - [SpeculateProjectorRequest](#angzarr-SpeculateProjectorRequest)
  
    - [ProjectorCoordinatorService](#angzarr-ProjectorCoordinatorService)
    - [ProjectorService](#angzarr-ProjectorService)
  
- [angzarr/query.proto](#angzarr_query-proto)
    - [EventQueryService](#angzarr-EventQueryService)
  
- [angzarr/saga.proto](#angzarr_saga-proto)
    - [SagaCompensationFailed](#angzarr-SagaCompensationFailed)
    - [SagaExecuteRequest](#angzarr-SagaExecuteRequest)
    - [SagaPrepareRequest](#angzarr-SagaPrepareRequest)
    - [SagaPrepareResponse](#angzarr-SagaPrepareResponse)
    - [SagaResponse](#angzarr-SagaResponse)
    - [SagaRetryRequest](#angzarr-SagaRetryRequest)
    - [SpeculateSagaRequest](#angzarr-SpeculateSagaRequest)
  
    - [SagaCoordinatorService](#angzarr-SagaCoordinatorService)
    - [SagaService](#angzarr-SagaService)
  
- [angzarr/stream.proto](#angzarr_stream-proto)
    - [EventStreamService](#angzarr-EventStreamService)
  
- [angzarr/types.proto](#angzarr_types-proto)
    - [AggregateRoot](#angzarr-AggregateRoot)
    - [AngzarrDeadLetter](#angzarr-AngzarrDeadLetter)
    - [AngzarrDeadLetter.MetadataEntry](#angzarr-AngzarrDeadLetter-MetadataEntry)
    - [CommandBook](#angzarr-CommandBook)
    - [CommandPage](#angzarr-CommandPage)
    - [ComponentDescriptor](#angzarr-ComponentDescriptor)
    - [ContextualCommand](#angzarr-ContextualCommand)
    - [Cover](#angzarr-Cover)
    - [DomainDivergence](#angzarr-DomainDivergence)
    - [Edition](#angzarr-Edition)
    - [EventBook](#angzarr-EventBook)
    - [EventPage](#angzarr-EventPage)
    - [EventProcessingFailedDetails](#angzarr-EventProcessingFailedDetails)
    - [EventStreamFilter](#angzarr-EventStreamFilter)
    - [GetDescriptorRequest](#angzarr-GetDescriptorRequest)
    - [Notification](#angzarr-Notification)
    - [Notification.MetadataEntry](#angzarr-Notification-MetadataEntry)
    - [PayloadReference](#angzarr-PayloadReference)
    - [PayloadRetrievalFailedDetails](#angzarr-PayloadRetrievalFailedDetails)
    - [Projection](#angzarr-Projection)
    - [Query](#angzarr-Query)
    - [RejectionNotification](#angzarr-RejectionNotification)
    - [SagaCommandOrigin](#angzarr-SagaCommandOrigin)
    - [SequenceMismatchDetails](#angzarr-SequenceMismatchDetails)
    - [SequenceRange](#angzarr-SequenceRange)
    - [SequenceSet](#angzarr-SequenceSet)
    - [Snapshot](#angzarr-Snapshot)
    - [SyncCommandBook](#angzarr-SyncCommandBook)
    - [SyncContextualCommand](#angzarr-SyncContextualCommand)
    - [SyncEventBook](#angzarr-SyncEventBook)
    - [Target](#angzarr-Target)
    - [TemporalQuery](#angzarr-TemporalQuery)
    - [UUID](#angzarr-UUID)
  
    - [MergeStrategy](#angzarr-MergeStrategy)
    - [PayloadStorageType](#angzarr-PayloadStorageType)
    - [SnapshotRetention](#angzarr-SnapshotRetention)
    - [SyncMode](#angzarr-SyncMode)
  
- [angzarr/upcaster.proto](#angzarr_upcaster-proto)
    - [UpcastRequest](#angzarr-UpcastRequest)
    - [UpcastResponse](#angzarr-UpcastResponse)
  
    - [UpcasterService](#angzarr-UpcasterService)
  
- [examples/ai_sidecar.proto](#examples_ai_sidecar-proto)
    - [ActionHistory](#examples-ActionHistory)
    - [ActionRequest](#examples-ActionRequest)
    - [ActionResponse](#examples-ActionResponse)
    - [BatchActionRequest](#examples-BatchActionRequest)
    - [BatchActionResponse](#examples-BatchActionResponse)
    - [HealthRequest](#examples-HealthRequest)
    - [HealthResponse](#examples-HealthResponse)
    - [OpponentStats](#examples-OpponentStats)
  
    - [AiSidecar](#examples-AiSidecar)
  
- [examples/hand.proto](#examples_hand-proto)
    - [ActionTaken](#examples-ActionTaken)
    - [AwardPot](#examples-AwardPot)
    - [BettingRoundComplete](#examples-BettingRoundComplete)
    - [BlindPosted](#examples-BlindPosted)
    - [CardsDealt](#examples-CardsDealt)
    - [CardsMucked](#examples-CardsMucked)
    - [CardsRevealed](#examples-CardsRevealed)
    - [CommunityCardsDealt](#examples-CommunityCardsDealt)
    - [DealCards](#examples-DealCards)
    - [DealCommunityCards](#examples-DealCommunityCards)
    - [DrawCompleted](#examples-DrawCompleted)
    - [HandComplete](#examples-HandComplete)
    - [HandState](#examples-HandState)
    - [PlayerAction](#examples-PlayerAction)
    - [PlayerHandState](#examples-PlayerHandState)
    - [PlayerHoleCards](#examples-PlayerHoleCards)
    - [PlayerInHand](#examples-PlayerInHand)
    - [PlayerStackSnapshot](#examples-PlayerStackSnapshot)
    - [PlayerTimedOut](#examples-PlayerTimedOut)
    - [PostBlind](#examples-PostBlind)
    - [PotAward](#examples-PotAward)
    - [PotAwarded](#examples-PotAwarded)
    - [PotWinner](#examples-PotWinner)
    - [RequestDraw](#examples-RequestDraw)
    - [RevealCards](#examples-RevealCards)
    - [ShowdownStarted](#examples-ShowdownStarted)
  
- [examples/player.proto](#examples_player-proto)
    - [ActionRequested](#examples-ActionRequested)
    - [DepositFunds](#examples-DepositFunds)
    - [FundsDeposited](#examples-FundsDeposited)
    - [FundsReleased](#examples-FundsReleased)
    - [FundsReserved](#examples-FundsReserved)
    - [FundsTransferred](#examples-FundsTransferred)
    - [FundsWithdrawn](#examples-FundsWithdrawn)
    - [PlayerRegistered](#examples-PlayerRegistered)
    - [PlayerState](#examples-PlayerState)
    - [PlayerState.TableReservationsEntry](#examples-PlayerState-TableReservationsEntry)
    - [RegisterPlayer](#examples-RegisterPlayer)
    - [ReleaseFunds](#examples-ReleaseFunds)
    - [RequestAction](#examples-RequestAction)
    - [ReserveFunds](#examples-ReserveFunds)
    - [TransferFunds](#examples-TransferFunds)
    - [WithdrawFunds](#examples-WithdrawFunds)
  
- [examples/poker_types.proto](#examples_poker_types-proto)
    - [Card](#examples-Card)
    - [Currency](#examples-Currency)
    - [HandRanking](#examples-HandRanking)
    - [Pot](#examples-Pot)
    - [Seat](#examples-Seat)
  
    - [ActionType](#examples-ActionType)
    - [BettingPhase](#examples-BettingPhase)
    - [GameVariant](#examples-GameVariant)
    - [HandRankType](#examples-HandRankType)
    - [PlayerType](#examples-PlayerType)
    - [Rank](#examples-Rank)
    - [Suit](#examples-Suit)
  
- [examples/table.proto](#examples_table-proto)
    - [AddChips](#examples-AddChips)
    - [ChipsAdded](#examples-ChipsAdded)
    - [CreateTable](#examples-CreateTable)
    - [EndHand](#examples-EndHand)
    - [HandEnded](#examples-HandEnded)
    - [HandEnded.StackChangesEntry](#examples-HandEnded-StackChangesEntry)
    - [HandStarted](#examples-HandStarted)
    - [JoinTable](#examples-JoinTable)
    - [LeaveTable](#examples-LeaveTable)
    - [PlayerJoined](#examples-PlayerJoined)
    - [PlayerLeft](#examples-PlayerLeft)
    - [PlayerSatIn](#examples-PlayerSatIn)
    - [PlayerSatOut](#examples-PlayerSatOut)
    - [PotResult](#examples-PotResult)
    - [SeatSnapshot](#examples-SeatSnapshot)
    - [SitIn](#examples-SitIn)
    - [SitOut](#examples-SitOut)
    - [StartHand](#examples-StartHand)
    - [TableCreated](#examples-TableCreated)
    - [TableState](#examples-TableState)
  
- [io/cloudevents/v1/cloudevents.proto](#io_cloudevents_v1_cloudevents-proto)
    - [CloudEvent](#io-cloudevents-v1-CloudEvent)
    - [CloudEvent.AttributesEntry](#io-cloudevents-v1-CloudEvent-AttributesEntry)
    - [CloudEventAttributeValue](#io-cloudevents-v1-CloudEventAttributeValue)
    - [CloudEventBatch](#io-cloudevents-v1-CloudEventBatch)
  
- [Scalar Value Types](#scalar-value-types)



<a name="angzarr_aggregate-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/aggregate.proto



<a name="angzarr-BusinessResponse"></a>

### BusinessResponse
Wrapper response for BusinessLogic.Handle


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  | Business provides compensation events |
| revocation | [RevocationResponse](#angzarr-RevocationResponse) |  | Business requests framework action |
| notification | [Notification](#angzarr-Notification) |  | Forward rejection notification upstream |






<a name="angzarr-CommandResponse"></a>

### CommandResponse
Response from entity - aggregate events &#43; sync projector results


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  | Events from the target aggregate |
| projections | [Projection](#angzarr-Projection) | repeated | Synchronous projector results |






<a name="angzarr-ReplayRequest"></a>

### ReplayRequest
Request to replay events and compute resulting state


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| base_snapshot | [Snapshot](#angzarr-Snapshot) |  | Starting state (empty = initial state) |
| events | [EventPage](#angzarr-EventPage) | repeated | Events to apply in order |






<a name="angzarr-ReplayResponse"></a>

### ReplayResponse
Response with computed state after replay


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| state | [google.protobuf.Any](#google-protobuf-Any) |  | Resulting state |






<a name="angzarr-RevocationResponse"></a>

### RevocationResponse
client logic requests framework to handle revocation


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| emit_system_revocation | [bool](#bool) |  | Emit SagaCompensationFailed event |
| send_to_dead_letter_queue | [bool](#bool) |  | Send to DLQ |
| escalate | [bool](#bool) |  | Flag for alerting/human intervention |
| abort | [bool](#bool) |  | Stop saga chain, propagate error to caller |
| reason | [string](#string) |  | Context/reason |






<a name="angzarr-SpeculateAggregateRequest"></a>

### SpeculateAggregateRequest
Request for speculative command execution against temporal state.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| command | [CommandBook](#angzarr-CommandBook) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 


<a name="angzarr-AggregateCoordinatorService"></a>

### AggregateCoordinatorService
AggregateCoordinatorService: orchestrates command processing for aggregates

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Handle | [CommandBook](#angzarr-CommandBook) | [CommandResponse](#angzarr-CommandResponse) | Async processing - fire and forget |
| HandleSync | [SyncCommandBook](#angzarr-SyncCommandBook) | [CommandResponse](#angzarr-CommandResponse) | Sync processing - waits for completion based on sync_mode |
| HandleSyncSpeculative | [SpeculateAggregateRequest](#angzarr-SpeculateAggregateRequest) | [CommandResponse](#angzarr-CommandResponse) | Speculative execution - execute against temporal state without persisting |
| HandleCompensation | [CommandBook](#angzarr-CommandBook) | [BusinessResponse](#angzarr-BusinessResponse) | Compensation flow - returns BusinessResponse for saga compensation handling. If business returns events, persists them. Caller handles revocation flags. |


<a name="angzarr-AggregateService"></a>

### AggregateService
AggregateService: client logic that processes commands and emits events
Also known as Command Handler in CQRS terminology
client logic doesn&#39;t care about sync - coordinator decides

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Handle | [ContextualCommand](#angzarr-ContextualCommand) | [BusinessResponse](#angzarr-BusinessResponse) | Process command and return business response (events or revocation request) |
| Replay | [ReplayRequest](#angzarr-ReplayRequest) | [ReplayResponse](#angzarr-ReplayResponse) | Replay events to compute state (for conflict detection) Optional: only needed if aggregate supports MERGE_COMMUTATIVE |

 



<a name="angzarr_cloudevents-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/cloudevents.proto



<a name="angzarr-CloudEvent"></a>

### CloudEvent
docs:start:cloud_event
CloudEvent represents a single event for external consumption.

Client projectors create these by filtering/transforming internal events.
Framework fills envelope fields (id, source, time) from Cover/EventPage
if not explicitly set by the client.

The `data` field is a protobuf Any that framework converts to JSON via
prost-reflect using the descriptor pool. Clients should pack a &#34;public&#34;
proto message that omits sensitive fields.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| type | [string](#string) |  | Event type (e.g., &#34;com.example.order.created&#34;). Default: proto type_url suffix from original event. |
| data | [google.protobuf.Any](#google-protobuf-Any) |  | Event payload as proto Any. Framework converts to JSON for CloudEvents output. Client should filter sensitive fields before packing. |
| extensions | [CloudEvent.ExtensionsEntry](#angzarr-CloudEvent-ExtensionsEntry) | repeated | Custom extension attributes. Keys should follow CloudEvents naming (lowercase, no dots). Framework adds correlationid automatically if present in Cover. |
| id | [string](#string) | optional | Optional overrides. Framework uses Cover/EventPage values if not set.

Default: `{domain}`:`{root_id}`:`{sequence}` |
| source | [string](#string) | optional | Default: angzarr/`{domain}` |
| subject | [string](#string) | optional | Default: aggregate root ID |






<a name="angzarr-CloudEvent-ExtensionsEntry"></a>

### CloudEvent.ExtensionsEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [string](#string) |  |  |
| value | [string](#string) |  |  |






<a name="angzarr-CloudEventsResponse"></a>

### CloudEventsResponse
CloudEventsResponse is returned by client projectors in Projection.projection.

Framework detects this type by checking projection.type_url and routes
the events to configured sinks (HTTP webhook, Kafka).

Client may return 0 events (skip), 1 event (typical), or N events
(fan-out scenarios like multi-tenant notifications).


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [CloudEvent](#angzarr-CloudEvent) | repeated |  |





 

 

 

 



<a name="angzarr_meta-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/meta.proto



<a name="angzarr-DeleteEditionEvents"></a>

### DeleteEditionEvents
Delete all events for an edition&#43;domain combination.
Main timeline (&#39;angzarr&#39; or empty edition name) cannot be deleted.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| edition | [string](#string) |  | Edition name to delete from |
| domain | [string](#string) |  | Domain to delete from |






<a name="angzarr-EditionEventsDeleted"></a>

### EditionEventsDeleted
Response from edition event deletion.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| edition | [string](#string) |  |  |
| domain | [string](#string) |  |  |
| deleted_count | [uint32](#uint32) |  |  |
| deleted_at | [string](#string) |  |  |





 

 

 

 



<a name="angzarr_process_manager-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/process_manager.proto



<a name="angzarr-ProcessManagerHandleRequest"></a>

### ProcessManagerHandleRequest
Phase 2 request: full context for PM decision.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| trigger | [EventBook](#angzarr-EventBook) |  | Full state of triggering domain. |
| process_state | [EventBook](#angzarr-EventBook) |  | Current process manager state (event-sourced). |
| destinations | [EventBook](#angzarr-EventBook) | repeated | Additional destinations fetched per Prepare response. |






<a name="angzarr-ProcessManagerHandleResponse"></a>

### ProcessManagerHandleResponse
Phase 2 response: commands and PM events.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| commands | [CommandBook](#angzarr-CommandBook) | repeated | Commands to issue to other aggregates. |
| process_events | [EventBook](#angzarr-EventBook) |  | Events for the process manager&#39;s own domain (non-duplicative workflow state). These are persisted via AggregateCoordinator to the PM&#39;s domain. |






<a name="angzarr-ProcessManagerPrepareRequest"></a>

### ProcessManagerPrepareRequest
Phase 1 request: PM declares additional destinations needed.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| trigger | [EventBook](#angzarr-EventBook) |  | Full state of triggering domain (by correlation_id). |
| process_state | [EventBook](#angzarr-EventBook) |  | Current process manager state (by correlation_id). May be empty for new workflow. |






<a name="angzarr-ProcessManagerPrepareResponse"></a>

### ProcessManagerPrepareResponse
Phase 1 response: destinations to fetch before Handle.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| destinations | [Cover](#angzarr-Cover) | repeated | Additional aggregates needed beyond trigger. Query by correlation_id. Minimize fetches - only declare what&#39;s actually needed. |






<a name="angzarr-SpeculatePmRequest"></a>

### SpeculatePmRequest
Request for speculative PM execution at a point in time.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| request | [ProcessManagerHandleRequest](#angzarr-ProcessManagerHandleRequest) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 


<a name="angzarr-ProcessManagerCoordinatorService"></a>

### ProcessManagerCoordinatorService
ProcessManagerCoordinatorService: orchestrates PM execution

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| HandleSpeculative | [SpeculatePmRequest](#angzarr-SpeculatePmRequest) | [ProcessManagerHandleResponse](#angzarr-ProcessManagerHandleResponse) | Speculative execution - returns commands and events without persisting |


<a name="angzarr-ProcessManagerService"></a>

### ProcessManagerService
ProcessManagerService: stateful coordinator for long-running workflows across multiple aggregates.

WARNING: Only use when saga &#43; queries is insufficient. Consider:
- Can a simple saga &#43; destination queries solve this?
- Is the &#34;state&#34; you want to track already derivable from existing aggregates?
- Are you adding Process Manager because the workflow is genuinely complex?

Process Manager is warranted when:
- Workflow state is NOT derivable from aggregates (PM owns unique state)
- You need to query workflow status independently (&#34;show all pending fulfillments&#34;)
- Timeout/scheduling logic is complex enough to merit its own aggregate
- You must react to events from MULTIPLE domains (saga recommends single domain)

Process Manager IS an aggregate with its own domain, events, and state.
It reuses all aggregate infrastructure (EventStore, SnapshotStore, AggregateCoordinator).

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Prepare | [ProcessManagerPrepareRequest](#angzarr-ProcessManagerPrepareRequest) | [ProcessManagerPrepareResponse](#angzarr-ProcessManagerPrepareResponse) | Phase 1: Declare which additional destinations are needed beyond the trigger. PM automatically receives triggering event&#39;s domain state. |
| Handle | [ProcessManagerHandleRequest](#angzarr-ProcessManagerHandleRequest) | [ProcessManagerHandleResponse](#angzarr-ProcessManagerHandleResponse) | Phase 2: Handle with trigger &#43; process state &#43; fetched destinations. Returns commands for other aggregates and events for the PM&#39;s own domain. |

 



<a name="angzarr_projector-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/projector.proto



<a name="angzarr-SpeculateProjectorRequest"></a>

### SpeculateProjectorRequest
Request for speculative projector execution at a point in time.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 


<a name="angzarr-ProjectorCoordinatorService"></a>

### ProjectorCoordinatorService
ProjectorCoordinatorService: orchestrates projection processing

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| HandleSync | [SyncEventBook](#angzarr-SyncEventBook) | [Projection](#angzarr-Projection) | Sync processing - returns projection based on sync_mode |
| Handle | [EventBook](#angzarr-EventBook) | [.google.protobuf.Empty](#google-protobuf-Empty) | Async processing - fire and forget |
| HandleSpeculative | [SpeculateProjectorRequest](#angzarr-SpeculateProjectorRequest) | [Projection](#angzarr-Projection) | Speculative processing - returns projection without side effects |


<a name="angzarr-ProjectorService"></a>

### ProjectorService
ProjectorService: client logic that projects events to read models
client logic doesn&#39;t care about sync - coordinator decides

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Handle | [EventBook](#angzarr-EventBook) | [Projection](#angzarr-Projection) | Async projection - projector should persist and return |
| HandleSpeculative | [EventBook](#angzarr-EventBook) | [Projection](#angzarr-Projection) | Speculative processing - projector must avoid external side effects |

 



<a name="angzarr_query-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/query.proto


 

 

 


<a name="angzarr-EventQueryService"></a>

### EventQueryService
EventQueryService: query interface for retrieving events

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| GetEventBook | [Query](#angzarr-Query) | [EventBook](#angzarr-EventBook) | Get a single EventBook (unary) - use for explicit queries with gRPC tooling |
| GetEvents | [Query](#angzarr-Query) | [EventBook](#angzarr-EventBook) stream | Stream EventBooks matching query - use for bulk retrieval |
| Synchronize | [Query](#angzarr-Query) stream | [EventBook](#angzarr-EventBook) stream |  |
| GetAggregateRoots | [.google.protobuf.Empty](#google-protobuf-Empty) | [AggregateRoot](#angzarr-AggregateRoot) stream |  |

 



<a name="angzarr_saga-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/saga.proto



<a name="angzarr-SagaCompensationFailed"></a>

### SagaCompensationFailed
System event when compensation fails/requested


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| triggering_aggregate | [Cover](#angzarr-Cover) |  |  |
| triggering_event_sequence | [uint32](#uint32) |  |  |
| saga_name | [string](#string) |  |  |
| rejection_reason | [string](#string) |  |  |
| compensation_failure_reason | [string](#string) |  |  |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  |  |
| occurred_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="angzarr-SagaExecuteRequest"></a>

### SagaExecuteRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [EventBook](#angzarr-EventBook) |  | Source events (same as prepare) |
| destinations | [EventBook](#angzarr-EventBook) | repeated | Fetched destination state |






<a name="angzarr-SagaPrepareRequest"></a>

### SagaPrepareRequest
Two-phase saga protocol messages


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [EventBook](#angzarr-EventBook) |  | Source events that triggered the saga |






<a name="angzarr-SagaPrepareResponse"></a>

### SagaPrepareResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| destinations | [Cover](#angzarr-Cover) | repeated | Destination aggregates the saga needs to read |






<a name="angzarr-SagaResponse"></a>

### SagaResponse
Response from saga - commands for other aggregates


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| commands | [CommandBook](#angzarr-CommandBook) | repeated | Commands to execute on other aggregates |
| events | [EventBook](#angzarr-EventBook) | repeated | Events to publish directly |






<a name="angzarr-SagaRetryRequest"></a>

### SagaRetryRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [EventBook](#angzarr-EventBook) |  |  |
| destinations | [EventBook](#angzarr-EventBook) | repeated |  |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  |  |
| rejection_reason | [string](#string) |  |  |
| attempt | [uint32](#uint32) |  |  |






<a name="angzarr-SpeculateSagaRequest"></a>

### SpeculateSagaRequest
Request for speculative saga execution at a point in time.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| request | [SagaExecuteRequest](#angzarr-SagaExecuteRequest) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 


<a name="angzarr-SagaCoordinatorService"></a>

### SagaCoordinatorService
SagaCoordinatorService: orchestrates saga execution

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Execute | [SagaExecuteRequest](#angzarr-SagaExecuteRequest) | [SagaResponse](#angzarr-SagaResponse) | Async processing - fire and forget |
| ExecuteSpeculative | [SpeculateSagaRequest](#angzarr-SpeculateSagaRequest) | [SagaResponse](#angzarr-SagaResponse) | Speculative execution - returns commands without side effects |


<a name="angzarr-SagaService"></a>

### SagaService
SagaService: client logic that coordinates across aggregates
Two-phase protocol: Prepare (declare destinations) → Execute (with fetched state)

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Prepare | [SagaPrepareRequest](#angzarr-SagaPrepareRequest) | [SagaPrepareResponse](#angzarr-SagaPrepareResponse) | Phase 1: Saga declares which destination aggregates it needs |
| Execute | [SagaExecuteRequest](#angzarr-SagaExecuteRequest) | [SagaResponse](#angzarr-SagaResponse) | Phase 2: Execute with source &#43; fetched destination state |

 



<a name="angzarr_stream-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/stream.proto


 

 

 


<a name="angzarr-EventStreamService"></a>

### EventStreamService
docs:start:event_stream_service
EventStreamService: streams events to registered subscribers

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Subscribe | [EventStreamFilter](#angzarr-EventStreamFilter) | [EventBook](#angzarr-EventBook) stream | Subscribe to events matching correlation ID (required) Returns INVALID_ARGUMENT if correlation_id is empty |

 



<a name="angzarr_types-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/types.proto



<a name="angzarr-AggregateRoot"></a>

### AggregateRoot



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | [string](#string) |  |  |
| root | [UUID](#angzarr-UUID) |  |  |






<a name="angzarr-AngzarrDeadLetter"></a>

### AngzarrDeadLetter
docs:start:dead_letter
Dead letter queue entry for failed messages requiring manual intervention.
Per-domain topics: angzarr.dlq.`{domain}`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  | Routing: domain, root, correlation_id |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  | Command that failed |
| rejected_events | [EventBook](#angzarr-EventBook) |  | Events that failed (saga/projector failures) |
| rejection_reason | [string](#string) |  | Human-readable reason |
| sequence_mismatch | [SequenceMismatchDetails](#angzarr-SequenceMismatchDetails) |  | Sequence conflict details |
| event_processing_failed | [EventProcessingFailedDetails](#angzarr-EventProcessingFailedDetails) |  | Handler failure details |
| payload_retrieval_failed | [PayloadRetrievalFailedDetails](#angzarr-PayloadRetrievalFailedDetails) |  | Payload store failure details |
| occurred_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |
| metadata | [AngzarrDeadLetter.MetadataEntry](#angzarr-AngzarrDeadLetter-MetadataEntry) | repeated | Additional context |
| source_component | [string](#string) |  | Which component sent to DLQ |
| source_component_type | [string](#string) |  | &#34;aggregate&#34; | &#34;saga&#34; | &#34;projector&#34; | &#34;process_manager&#34; |






<a name="angzarr-AngzarrDeadLetter-MetadataEntry"></a>

### AngzarrDeadLetter.MetadataEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [string](#string) |  |  |
| value | [string](#string) |  |  |






<a name="angzarr-CommandBook"></a>

### CommandBook



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  |  |
| pages | [CommandPage](#angzarr-CommandPage) | repeated |  |
| saga_origin | [SagaCommandOrigin](#angzarr-SagaCommandOrigin) |  | Field 3 removed: correlation_id moved to Cover

Tracks origin for compensation flow |






<a name="angzarr-CommandPage"></a>

### CommandPage



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sequence | [uint32](#uint32) |  | Expected sequence number for this command&#39;s events. Must match the aggregate&#39;s current next sequence (i.e., events.len()). For new aggregates, use 0. |
| merge_strategy | [MergeStrategy](#angzarr-MergeStrategy) |  |  |
| command | [google.protobuf.Any](#google-protobuf-Any) |  |  |
| external | [PayloadReference](#angzarr-PayloadReference) |  | Claim check: payload stored externally |






<a name="angzarr-ComponentDescriptor"></a>

### ComponentDescriptor
Component self-description.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  |  |
| component_type | [string](#string) |  |  |
| inputs | [Target](#angzarr-Target) | repeated | Domains I subscribe to (event types I consume) |






<a name="angzarr-ContextualCommand"></a>

### ContextualCommand



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  | Passed from aggregate coordinator to aggregate, consists of everything needed to execute/evaluate the command |
| command | [CommandBook](#angzarr-CommandBook) |  |  |






<a name="angzarr-Cover"></a>

### Cover
docs:start:cover


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | [string](#string) |  |  |
| root | [UUID](#angzarr-UUID) |  |  |
| correlation_id | [string](#string) |  | Workflow correlation - flows through all commands/events |
| edition | [Edition](#angzarr-Edition) |  | Edition for diverged timelines; empty name = main timeline |






<a name="angzarr-DomainDivergence"></a>

### DomainDivergence
Explicit divergence point for a specific domain.
Used when creating historical branches or coordinating saga writes across domains.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | [string](#string) |  | Domain name |
| sequence | [uint32](#uint32) |  | Divergence sequence number |






<a name="angzarr-Edition"></a>

### Edition
docs:start:edition
Edition identifier with optional explicit divergence points.

Two modes:
- Implicit (divergences empty): Divergence derived from first edition event&#39;s sequence
- Explicit (divergences populated): Per-domain divergence points for historical branching,
 saga coordination, or speculative execution


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | [string](#string) |  | Edition name, e.g., &#34;v2&#34;; empty = main timeline |
| divergences | [DomainDivergence](#angzarr-DomainDivergence) | repeated | Optional: explicit per-domain divergence points |






<a name="angzarr-EventBook"></a>

### EventBook
docs:start:event_book


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  |  |
| snapshot | [Snapshot](#angzarr-Snapshot) |  | Snapshot state; sequence computed by framework on persist |
| pages | [EventPage](#angzarr-EventPage) | repeated |  |
| next_sequence | [uint32](#uint32) |  | Field 4 removed: correlation_id moved to Cover Field 5 removed: snapshot_state unified into snapshot field

Computed on load, never stored: (last page seq OR snapshot seq if no pages) &#43; 1 |






<a name="angzarr-EventPage"></a>

### EventPage
docs:start:event_page


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sequence | [uint32](#uint32) |  |  |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |
| event | [google.protobuf.Any](#google-protobuf-Any) |  |  |
| external | [PayloadReference](#angzarr-PayloadReference) |  | Claim check: payload stored externally |






<a name="angzarr-EventProcessingFailedDetails"></a>

### EventProcessingFailedDetails
Event processing failure details for DLQ entries.
Contains information about why a saga/projector failed to process events.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| error | [string](#string) |  | Error message from the handler |
| retry_count | [uint32](#uint32) |  | Number of retry attempts before DLQ routing |
| is_transient | [bool](#bool) |  | Whether the failure is considered transient |






<a name="angzarr-EventStreamFilter"></a>

### EventStreamFilter
docs:start:event_stream_filter
Subscription filter for event streaming


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| correlation_id | [string](#string) |  |  |






<a name="angzarr-GetDescriptorRequest"></a>

### GetDescriptorRequest
Request for GetDescriptor RPC.






<a name="angzarr-Notification"></a>

### Notification
docs:start:notification
Base notification message for transient system signals.
Contains routing info via Cover but no persistence semantics.
Type discrimination via payload.type_url (standard Any behavior).


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  | Routing: domain, root, correlation_id |
| payload | [google.protobuf.Any](#google-protobuf-Any) |  | Type-specific content (RejectionNotification, etc.) |
| sent_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  | When notification was created |
| metadata | [Notification.MetadataEntry](#angzarr-Notification-MetadataEntry) | repeated | Optional key-value metadata |






<a name="angzarr-Notification-MetadataEntry"></a>

### Notification.MetadataEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [string](#string) |  |  |
| value | [string](#string) |  |  |






<a name="angzarr-PayloadReference"></a>

### PayloadReference
Reference to externally stored payload (claim check pattern).
Used when event/command payloads exceed message bus size limits.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| storage_type | [PayloadStorageType](#angzarr-PayloadStorageType) |  |  |
| uri | [string](#string) |  | Location URI: - file:///var/angzarr/payloads/`{hash}`.bin - gs://bucket/prefix/`{hash}`.bin - s3://bucket/prefix/`{hash}`.bin |
| content_hash | [bytes](#bytes) |  | Content hash for integrity verification and deduplication (SHA-256) |
| original_size | [uint64](#uint64) |  | Original serialized payload size in bytes |
| stored_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  | Timestamp when payload was stored (for TTL cleanup) |






<a name="angzarr-PayloadRetrievalFailedDetails"></a>

### PayloadRetrievalFailedDetails
Payload retrieval failure details for DLQ entries.
Contains information about why an externally stored payload couldn&#39;t be retrieved.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| storage_type | [PayloadStorageType](#angzarr-PayloadStorageType) |  | Storage backend type |
| uri | [string](#string) |  | URI of the payload that couldn&#39;t be retrieved |
| content_hash | [bytes](#bytes) |  | Content hash for identification |
| original_size | [uint64](#uint64) |  | Original payload size in bytes |
| error | [string](#string) |  | Error message from the retrieval attempt |






<a name="angzarr-Projection"></a>

### Projection



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  |  |
| projector | [string](#string) |  |  |
| sequence | [uint32](#uint32) |  |  |
| projection | [google.protobuf.Any](#google-protobuf-Any) |  |  |






<a name="angzarr-Query"></a>

### Query



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  | Cover identifies the aggregate: domain &#43; (root | correlation_id | both) Query by root: Cover `{ domain, root }` Query by correlation: Cover `{ domain, correlation_id }` |
| range | [SequenceRange](#angzarr-SequenceRange) |  |  |
| sequences | [SequenceSet](#angzarr-SequenceSet) |  |  |
| temporal | [TemporalQuery](#angzarr-TemporalQuery) |  |  |






<a name="angzarr-RejectionNotification"></a>

### RejectionNotification
docs:start:rejection_notification
Notification payload for command rejection scenarios.
Embedded in Notification.payload when a saga/PM command is rejected.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  | The command that was rejected (full context) |
| rejection_reason | [string](#string) |  | Why: &#34;insufficient_funds&#34;, &#34;out_of_stock&#34;, etc. |
| issuer_name | [string](#string) |  | Saga/PM name that issued the command |
| issuer_type | [string](#string) |  | &#34;saga&#34; | &#34;process_manager&#34; |
| source_aggregate | [Cover](#angzarr-Cover) |  | Aggregate that originally triggered the flow |
| source_event_sequence | [uint32](#uint32) |  | Event sequence that triggered the saga/PM |






<a name="angzarr-SagaCommandOrigin"></a>

### SagaCommandOrigin
Track saga command origin for compensation flow


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| saga_name | [string](#string) |  | Name of the saga that issued the command |
| triggering_aggregate | [Cover](#angzarr-Cover) |  | Domain&#43;root of aggregate that triggered the saga |
| triggering_event_sequence | [uint32](#uint32) |  | Sequence number of the triggering event |






<a name="angzarr-SequenceMismatchDetails"></a>

### SequenceMismatchDetails
docs:start:dlq_details
Sequence mismatch details for DLQ entries.
Contains expected vs actual sequence for debugging and replay.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| expected_sequence | [uint32](#uint32) |  | What the command expected |
| actual_sequence | [uint32](#uint32) |  | What the aggregate was at |
| merge_strategy | [MergeStrategy](#angzarr-MergeStrategy) |  | Strategy that triggered DLQ routing |






<a name="angzarr-SequenceRange"></a>

### SequenceRange
Query types


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| lower | [uint32](#uint32) |  |  |
| upper | [uint32](#uint32) | optional | If not set, query to latest |






<a name="angzarr-SequenceSet"></a>

### SequenceSet



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| values | [uint32](#uint32) | repeated |  |






<a name="angzarr-Snapshot"></a>

### Snapshot
docs:start:aggregate_snapshot
Snapshot of aggregate state at a given sequence number.
State must be a protobuf Message to serialize into Any.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sequence | [uint32](#uint32) |  |  |
| state | [google.protobuf.Any](#google-protobuf-Any) |  |  |
| retention | [SnapshotRetention](#angzarr-SnapshotRetention) |  | Controls cleanup behavior |






<a name="angzarr-SyncCommandBook"></a>

### SyncCommandBook



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| command | [CommandBook](#angzarr-CommandBook) |  |  |
| sync_mode | [SyncMode](#angzarr-SyncMode) |  |  |






<a name="angzarr-SyncContextualCommand"></a>

### SyncContextualCommand



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| command | [ContextualCommand](#angzarr-ContextualCommand) |  |  |
| sync_mode | [SyncMode](#angzarr-SyncMode) |  |  |






<a name="angzarr-SyncEventBook"></a>

### SyncEventBook



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  |  |
| sync_mode | [SyncMode](#angzarr-SyncMode) |  |  |






<a name="angzarr-Target"></a>

### Target
Describes what a component subscribes to.
Topology edges derived from inputs: if A subscribes to domain X, edge X→A exists.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | [string](#string) |  |  |






<a name="angzarr-TemporalQuery"></a>

### TemporalQuery
Temporal query: retrieve aggregate state at a point in history.
Replays events from sequence 0 (no snapshots) to the specified point.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| as_of_time | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  | Events with created_at &lt;= this |
| as_of_sequence | [uint32](#uint32) |  | Events with sequence &lt;= this |






<a name="angzarr-UUID"></a>

### UUID



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| value | [bytes](#bytes) |  |  |





 


<a name="angzarr-MergeStrategy"></a>

### MergeStrategy
docs:start:merge_strategy
Controls how concurrent commands to the same aggregate are handled

| Name | Number | Description |
| ---- | ------ | ----------- |
| MERGE_COMMUTATIVE | 0 | Default: allow if state field mutations don&#39;t overlap |
| MERGE_STRICT | 1 | Reject if sequence mismatch (optimistic concurrency) |
| MERGE_AGGREGATE_HANDLES | 2 | Aggregate handles its own concurrency |
| MERGE_MANUAL | 3 | Send to DLQ for manual review on mismatch |



<a name="angzarr-PayloadStorageType"></a>

### PayloadStorageType
docs:start:payload_reference
Storage backend type for externally stored payloads (claim check pattern).

| Name | Number | Description |
| ---- | ------ | ----------- |
| PAYLOAD_STORAGE_TYPE_UNSPECIFIED | 0 |  |
| PAYLOAD_STORAGE_TYPE_FILESYSTEM | 1 |  |
| PAYLOAD_STORAGE_TYPE_GCS | 2 |  |
| PAYLOAD_STORAGE_TYPE_S3 | 3 |  |



<a name="angzarr-SnapshotRetention"></a>

### SnapshotRetention
docs:start:snapshot_retention
Controls snapshot retention during cleanup

| Name | Number | Description |
| ---- | ------ | ----------- |
| RETENTION_DEFAULT | 0 | Persist every 16 events, treated as TRANSIENT otherwise |
| RETENTION_PERSIST | 1 | Keep indefinitely (business milestone) |
| RETENTION_TRANSIENT | 2 | Delete when newer snapshot written |



<a name="angzarr-SyncMode"></a>

### SyncMode
docs:start:sync_mode
Controls synchronous processing behavior

| Name | Number | Description |
| ---- | ------ | ----------- |
| SYNC_MODE_UNSPECIFIED | 0 | Async: fire and forget (default) |
| SYNC_MODE_SIMPLE | 1 | Sync projectors only, no saga cascade |
| SYNC_MODE_CASCADE | 2 | Full sync: projectors &#43; saga cascade (expensive) |


 

 

 



<a name="angzarr_upcaster-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## angzarr/upcaster.proto



<a name="angzarr-UpcastRequest"></a>

### UpcastRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | [string](#string) |  |  |
| events | [EventPage](#angzarr-EventPage) | repeated |  |






<a name="angzarr-UpcastResponse"></a>

### UpcastResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventPage](#angzarr-EventPage) | repeated |  |





 

 

 


<a name="angzarr-UpcasterService"></a>

### UpcasterService
UpcasterService: transforms old event versions to current versions
Implemented by the client alongside AggregateService on the same gRPC server.
Optionally can be deployed as a separate binary for testing or complex migrations.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Upcast | [UpcastRequest](#angzarr-UpcastRequest) | [UpcastResponse](#angzarr-UpcastResponse) | Transform events to current version Returns events in same order, transformed where applicable |

 



<a name="examples_ai_sidecar-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## examples/ai_sidecar.proto



<a name="examples-ActionHistory"></a>

### ActionHistory



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| action | [ActionType](#examples-ActionType) |  |  |
| amount | [int64](#int64) |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |






<a name="examples-ActionRequest"></a>

### ActionRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| model_id | [string](#string) |  | Game state

Which model to use |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |
| hole_cards | [Card](#examples-Card) | repeated | Cards |
| community_cards | [Card](#examples-Card) | repeated |  |
| pot_size | [int64](#int64) |  | Betting context |
| stack_size | [int64](#int64) |  |  |
| amount_to_call | [int64](#int64) |  |  |
| min_raise | [int64](#int64) |  |  |
| max_raise | [int64](#int64) |  |  |
| position | [int32](#int32) |  | Position info

0 = button, increasing = earlier |
| players_remaining | [int32](#int32) |  |  |
| players_to_act | [int32](#int32) |  |  |
| action_history | [ActionHistory](#examples-ActionHistory) | repeated | Historical context (for recurrent models) |
| opponents | [OpponentStats](#examples-OpponentStats) | repeated | Opponent modeling (optional) |






<a name="examples-ActionResponse"></a>

### ActionResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| recommended_action | [ActionType](#examples-ActionType) |  |  |
| amount | [int64](#int64) |  | For bet/raise |
| fold_probability | [float](#float) |  | Confidence scores for each action (for analysis) |
| check_call_probability | [float](#float) |  |  |
| bet_raise_probability | [float](#float) |  |  |
| model_version | [string](#string) |  | Model metadata |
| inference_time_ms | [int64](#int64) |  |  |






<a name="examples-BatchActionRequest"></a>

### BatchActionRequest



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| requests | [ActionRequest](#examples-ActionRequest) | repeated |  |






<a name="examples-BatchActionResponse"></a>

### BatchActionResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| responses | [ActionResponse](#examples-ActionResponse) | repeated |  |






<a name="examples-HealthRequest"></a>

### HealthRequest







<a name="examples-HealthResponse"></a>

### HealthResponse



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| healthy | [bool](#bool) |  |  |
| model_id | [string](#string) |  |  |
| model_version | [string](#string) |  |  |
| uptime_seconds | [int64](#int64) |  |  |
| requests_served | [int64](#int64) |  |  |






<a name="examples-OpponentStats"></a>

### OpponentStats



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| position | [int32](#int32) |  |  |
| stack | [int64](#int64) |  |  |
| vpip | [float](#float) |  | Voluntarily put in pot % |
| pfr | [float](#float) |  | Pre-flop raise % |
| aggression | [float](#float) |  | Bet/raise frequency |
| hands_played | [int32](#int32) |  |  |





 

 

 


<a name="examples-AiSidecar"></a>

### AiSidecar


| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| GetAction | [ActionRequest](#examples-ActionRequest) | [ActionResponse](#examples-ActionResponse) | Get recommended action from the AI model |
| Health | [HealthRequest](#examples-HealthRequest) | [HealthResponse](#examples-HealthResponse) | Health check |
| GetActionsBatch | [BatchActionRequest](#examples-BatchActionRequest) | [BatchActionResponse](#examples-BatchActionResponse) | Batch inference for training/simulation |

 



<a name="examples_hand-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## examples/hand.proto



<a name="examples-ActionTaken"></a>

### ActionTaken



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| action | [ActionType](#examples-ActionType) |  |  |
| amount | [int64](#int64) |  |  |
| player_stack | [int64](#int64) |  | Absolute stack after action |
| pot_total | [int64](#int64) |  | Absolute pot after action |
| amount_to_call | [int64](#int64) |  | Current call amount for next player |
| action_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-AwardPot"></a>

### AwardPot



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| awards | [PotAward](#examples-PotAward) | repeated |  |






<a name="examples-BettingRoundComplete"></a>

### BettingRoundComplete



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| completed_phase | [BettingPhase](#examples-BettingPhase) |  |  |
| pot_total | [int64](#int64) |  |  |
| stacks | [PlayerStackSnapshot](#examples-PlayerStackSnapshot) | repeated |  |
| completed_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-BlindPosted"></a>

### BlindPosted



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| blind_type | [string](#string) |  |  |
| amount | [int64](#int64) |  |  |
| player_stack | [int64](#int64) |  | Absolute stack after posting |
| pot_total | [int64](#int64) |  | Absolute pot after posting |
| posted_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-CardsDealt"></a>

### CardsDealt



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | [bytes](#bytes) |  |  |
| hand_number | [int64](#int64) |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| player_cards | [PlayerHoleCards](#examples-PlayerHoleCards) | repeated |  |
| dealer_position | [int32](#int32) |  |  |
| players | [PlayerInHand](#examples-PlayerInHand) | repeated |  |
| dealt_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |
| remaining_deck | [Card](#examples-Card) | repeated | Cards left after dealing hole cards |






<a name="examples-CardsMucked"></a>

### CardsMucked



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| mucked_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-CardsRevealed"></a>

### CardsRevealed



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| cards | [Card](#examples-Card) | repeated |  |
| ranking | [HandRanking](#examples-HandRanking) |  |  |
| revealed_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-CommunityCardsDealt"></a>

### CommunityCardsDealt



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cards | [Card](#examples-Card) | repeated |  |
| phase | [BettingPhase](#examples-BettingPhase) |  | FLOP, TURN, or RIVER |
| all_community_cards | [Card](#examples-Card) | repeated | Full board so far |
| dealt_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-DealCards"></a>

### DealCards



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | [bytes](#bytes) |  |  |
| hand_number | [int64](#int64) |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| players | [PlayerInHand](#examples-PlayerInHand) | repeated |  |
| dealer_position | [int32](#int32) |  |  |
| small_blind | [int64](#int64) |  |  |
| big_blind | [int64](#int64) |  |  |
| deck_seed | [bytes](#bytes) |  | For deterministic shuffle (testing) |






<a name="examples-DealCommunityCards"></a>

### DealCommunityCards



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| count | [int32](#int32) |  | 3 for flop, 1 for turn/river |






<a name="examples-DrawCompleted"></a>

### DrawCompleted



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| cards_discarded | [int32](#int32) |  |  |
| cards_drawn | [int32](#int32) |  |  |
| new_cards | [Card](#examples-Card) | repeated | Only visible to this player |
| drawn_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-HandComplete"></a>

### HandComplete



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | [bytes](#bytes) |  |  |
| hand_number | [int64](#int64) |  |  |
| winners | [PotWinner](#examples-PotWinner) | repeated |  |
| final_stacks | [PlayerStackSnapshot](#examples-PlayerStackSnapshot) | repeated |  |
| completed_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-HandState"></a>

### HandState
State (for snapshots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_id | [string](#string) |  |  |
| table_root | [bytes](#bytes) |  |  |
| hand_number | [int64](#int64) |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| remaining_deck | [Card](#examples-Card) | repeated | Deck state |
| players | [PlayerHandState](#examples-PlayerHandState) | repeated | Player state |
| community_cards | [Card](#examples-Card) | repeated | Community cards |
| current_phase | [BettingPhase](#examples-BettingPhase) |  | Betting state |
| action_on_position | [int32](#int32) |  |  |
| current_bet | [int64](#int64) |  |  |
| min_raise | [int64](#int64) |  |  |
| pots | [Pot](#examples-Pot) | repeated |  |
| dealer_position | [int32](#int32) |  | Positions |
| small_blind_position | [int32](#int32) |  |  |
| big_blind_position | [int32](#int32) |  |  |
| status | [string](#string) |  | &#34;dealing&#34;, &#34;betting&#34;, &#34;showdown&#34;, &#34;complete&#34; |






<a name="examples-PlayerAction"></a>

### PlayerAction



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| action | [ActionType](#examples-ActionType) |  |  |
| amount | [int64](#int64) |  | For bet/raise/call |






<a name="examples-PlayerHandState"></a>

### PlayerHandState



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| position | [int32](#int32) |  |  |
| hole_cards | [Card](#examples-Card) | repeated |  |
| stack | [int64](#int64) |  |  |
| bet_this_round | [int64](#int64) |  |  |
| total_invested | [int64](#int64) |  |  |
| has_acted | [bool](#bool) |  |  |
| has_folded | [bool](#bool) |  |  |
| is_all_in | [bool](#bool) |  |  |






<a name="examples-PlayerHoleCards"></a>

### PlayerHoleCards



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| cards | [Card](#examples-Card) | repeated |  |






<a name="examples-PlayerInHand"></a>

### PlayerInHand



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| position | [int32](#int32) |  |  |
| stack | [int64](#int64) |  |  |






<a name="examples-PlayerStackSnapshot"></a>

### PlayerStackSnapshot



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| stack | [int64](#int64) |  |  |
| is_all_in | [bool](#bool) |  |  |
| has_folded | [bool](#bool) |  |  |






<a name="examples-PlayerTimedOut"></a>

### PlayerTimedOut



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| default_action | [ActionType](#examples-ActionType) |  | Usually FOLD or CHECK |
| timed_out_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PostBlind"></a>

### PostBlind



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| blind_type | [string](#string) |  | &#34;small&#34;, &#34;big&#34;, &#34;ante&#34; |
| amount | [int64](#int64) |  |  |






<a name="examples-PotAward"></a>

### PotAward



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| amount | [int64](#int64) |  |  |
| pot_type | [string](#string) |  |  |






<a name="examples-PotAwarded"></a>

### PotAwarded



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| winners | [PotWinner](#examples-PotWinner) | repeated |  |
| awarded_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PotWinner"></a>

### PotWinner



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| amount | [int64](#int64) |  |  |
| pot_type | [string](#string) |  |  |
| winning_hand | [HandRanking](#examples-HandRanking) |  |  |






<a name="examples-RequestDraw"></a>

### RequestDraw



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| card_indices | [int32](#int32) | repeated | Which cards to discard (0-indexed) |






<a name="examples-RevealCards"></a>

### RevealCards



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| muck | [bool](#bool) |  | True to hide cards (fold at showdown) |






<a name="examples-ShowdownStarted"></a>

### ShowdownStarted



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| players_to_show | [bytes](#bytes) | repeated | Order of revelation |
| started_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |





 

 

 

 



<a name="examples_player-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## examples/player.proto



<a name="examples-ActionRequested"></a>

### ActionRequested
Emitted when action is needed - AI players respond via sidecar


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | [bytes](#bytes) |  |  |
| table_root | [bytes](#bytes) |  |  |
| player_root | [bytes](#bytes) |  |  |
| player_type | [PlayerType](#examples-PlayerType) |  |  |
| amount_to_call | [int64](#int64) |  |  |
| min_raise | [int64](#int64) |  |  |
| max_raise | [int64](#int64) |  |  |
| hole_cards | [Card](#examples-Card) | repeated |  |
| community_cards | [Card](#examples-Card) | repeated |  |
| pot_size | [int64](#int64) |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |
| deadline | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-DepositFunds"></a>

### DepositFunds



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |






<a name="examples-FundsDeposited"></a>

### FundsDeposited



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| new_balance | [Currency](#examples-Currency) |  | Absolute value after deposit |
| deposited_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-FundsReleased"></a>

### FundsReleased



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| table_root | [bytes](#bytes) |  |  |
| new_available_balance | [Currency](#examples-Currency) |  |  |
| new_reserved_balance | [Currency](#examples-Currency) |  |  |
| released_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-FundsReserved"></a>

### FundsReserved



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| table_root | [bytes](#bytes) |  |  |
| new_available_balance | [Currency](#examples-Currency) |  | Bankroll minus reserved |
| new_reserved_balance | [Currency](#examples-Currency) |  | Total reserved across tables |
| reserved_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-FundsTransferred"></a>

### FundsTransferred



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| from_player_root | [bytes](#bytes) |  |  |
| to_player_root | [bytes](#bytes) |  |  |
| amount | [Currency](#examples-Currency) |  |  |
| hand_root | [bytes](#bytes) |  |  |
| reason | [string](#string) |  |  |
| new_balance | [Currency](#examples-Currency) |  | Recipient&#39;s new balance |
| transferred_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-FundsWithdrawn"></a>

### FundsWithdrawn



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| new_balance | [Currency](#examples-Currency) |  | Absolute value after withdrawal |
| withdrawn_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PlayerRegistered"></a>

### PlayerRegistered



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| display_name | [string](#string) |  |  |
| email | [string](#string) |  |  |
| player_type | [PlayerType](#examples-PlayerType) |  |  |
| ai_model_id | [string](#string) |  |  |
| registered_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PlayerState"></a>

### PlayerState
State (for snapshots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_id | [string](#string) |  |  |
| display_name | [string](#string) |  |  |
| email | [string](#string) |  |  |
| player_type | [PlayerType](#examples-PlayerType) |  |  |
| ai_model_id | [string](#string) |  |  |
| bankroll | [Currency](#examples-Currency) |  |  |
| reserved_funds | [Currency](#examples-Currency) |  |  |
| table_reservations | [PlayerState.TableReservationsEntry](#examples-PlayerState-TableReservationsEntry) | repeated | table_root_hex -&gt; amount |
| status | [string](#string) |  | &#34;active&#34;, &#34;suspended&#34;, etc. |






<a name="examples-PlayerState-TableReservationsEntry"></a>

### PlayerState.TableReservationsEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [string](#string) |  |  |
| value | [int64](#int64) |  |  |






<a name="examples-RegisterPlayer"></a>

### RegisterPlayer



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| display_name | [string](#string) |  |  |
| email | [string](#string) |  | Used for root derivation |
| player_type | [PlayerType](#examples-PlayerType) |  | HUMAN or AI |
| ai_model_id | [string](#string) |  | For AI players: which model to use |






<a name="examples-ReleaseFunds"></a>

### ReleaseFunds
Release reserved funds back to bankroll (leave table)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | [bytes](#bytes) |  |  |






<a name="examples-RequestAction"></a>

### RequestAction
Request action from player (triggers AI sidecar for AI players)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | [bytes](#bytes) |  |  |
| table_root | [bytes](#bytes) |  |  |
| amount_to_call | [int64](#int64) |  |  |
| min_raise | [int64](#int64) |  |  |
| max_raise | [int64](#int64) |  | Player&#39;s remaining stack |
| hole_cards | [Card](#examples-Card) | repeated |  |
| community_cards | [Card](#examples-Card) | repeated |  |
| pot_size | [int64](#int64) |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |
| timeout_seconds | [int32](#int32) |  |  |






<a name="examples-ReserveFunds"></a>

### ReserveFunds
Reserve funds when joining a table (buy-in)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| table_root | [bytes](#bytes) |  | Which table the funds are reserved for |






<a name="examples-TransferFunds"></a>

### TransferFunds
Transfer funds from one player to another (pot award)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| from_player_root | [bytes](#bytes) |  | Source player (for reserved funds) |
| amount | [Currency](#examples-Currency) |  |  |
| hand_root | [bytes](#bytes) |  | Which hand this transfer is for |
| reason | [string](#string) |  | &#34;pot_win&#34;, &#34;side_pot_win&#34;, etc. |






<a name="examples-WithdrawFunds"></a>

### WithdrawFunds



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |





 

 

 

 



<a name="examples_poker_types-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## examples/poker_types.proto



<a name="examples-Card"></a>

### Card
Card representation


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| suit | [Suit](#examples-Suit) |  |  |
| rank | [Rank](#examples-Rank) |  |  |






<a name="examples-Currency"></a>

### Currency
Currency amount (in smallest unit, e.g., cents)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [int64](#int64) |  |  |
| currency_code | [string](#string) |  | &#34;USD&#34;, &#34;EUR&#34;, &#34;CHIPS&#34; |






<a name="examples-HandRanking"></a>

### HandRanking
Hand ranking result


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| rank_type | [HandRankType](#examples-HandRankType) |  |  |
| kickers | [Rank](#examples-Rank) | repeated | For tie-breaking |
| score | [int32](#int32) |  | Numeric score for comparison |






<a name="examples-Pot"></a>

### Pot
Pot structure (for side pots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [int64](#int64) |  |  |
| eligible_players | [bytes](#bytes) | repeated | Player roots eligible for this pot |
| pot_type | [string](#string) |  | &#34;main&#34; or &#34;side_N&#34; |






<a name="examples-Seat"></a>

### Seat
Position at table


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| position | [int32](#int32) |  | 0-9 for 10-max table |
| player_root | [bytes](#bytes) |  | Player aggregate root |
| stack | [Currency](#examples-Currency) |  | Current stack at table |
| is_active | [bool](#bool) |  | Still in current hand |
| is_sitting_out | [bool](#bool) |  | Temporarily away |





 


<a name="examples-ActionType"></a>

### ActionType
Player action type

| Name | Number | Description |
| ---- | ------ | ----------- |
| ACTION_UNSPECIFIED | 0 |  |
| FOLD | 1 |  |
| CHECK | 2 |  |
| CALL | 3 |  |
| BET | 4 |  |
| RAISE | 5 |  |
| ALL_IN | 6 |  |



<a name="examples-BettingPhase"></a>

### BettingPhase
Betting round phase

| Name | Number | Description |
| ---- | ------ | ----------- |
| BETTING_PHASE_UNSPECIFIED | 0 |  |
| PREFLOP | 1 |  |
| FLOP | 2 |  |
| TURN | 3 |  |
| RIVER | 4 |  |
| DRAW | 5 | For draw games |
| SHOWDOWN | 6 |  |



<a name="examples-GameVariant"></a>

### GameVariant
Game variant configuration

| Name | Number | Description |
| ---- | ------ | ----------- |
| GAME_VARIANT_UNSPECIFIED | 0 |  |
| TEXAS_HOLDEM | 1 |  |
| OMAHA | 2 |  |
| FIVE_CARD_DRAW | 3 |  |
| SEVEN_CARD_STUD | 4 |  |



<a name="examples-HandRankType"></a>

### HandRankType


| Name | Number | Description |
| ---- | ------ | ----------- |
| HAND_RANK_UNSPECIFIED | 0 |  |
| HIGH_CARD | 1 |  |
| PAIR | 2 |  |
| TWO_PAIR | 3 |  |
| THREE_OF_A_KIND | 4 |  |
| STRAIGHT | 5 |  |
| FLUSH | 6 |  |
| FULL_HOUSE | 7 |  |
| FOUR_OF_A_KIND | 8 |  |
| STRAIGHT_FLUSH | 9 |  |
| ROYAL_FLUSH | 10 |  |



<a name="examples-PlayerType"></a>

### PlayerType
Player type - abstraction for human vs AI

| Name | Number | Description |
| ---- | ------ | ----------- |
| PLAYER_TYPE_UNSPECIFIED | 0 |  |
| HUMAN | 1 |  |
| AI | 2 |  |



<a name="examples-Rank"></a>

### Rank


| Name | Number | Description |
| ---- | ------ | ----------- |
| RANK_UNSPECIFIED | 0 |  |
| TWO | 2 |  |
| THREE | 3 |  |
| FOUR | 4 |  |
| FIVE | 5 |  |
| SIX | 6 |  |
| SEVEN | 7 |  |
| EIGHT | 8 |  |
| NINE | 9 |  |
| TEN | 10 |  |
| JACK | 11 |  |
| QUEEN | 12 |  |
| KING | 13 |  |
| ACE | 14 |  |



<a name="examples-Suit"></a>

### Suit


| Name | Number | Description |
| ---- | ------ | ----------- |
| SUIT_UNSPECIFIED | 0 |  |
| CLUBS | 1 |  |
| DIAMONDS | 2 |  |
| HEARTS | 3 |  |
| SPADES | 4 |  |


 

 

 



<a name="examples_table-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## examples/table.proto



<a name="examples-AddChips"></a>

### AddChips



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| amount | [int64](#int64) |  |  |






<a name="examples-ChipsAdded"></a>

### ChipsAdded



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| amount | [int64](#int64) |  |  |
| new_stack | [int64](#int64) |  | Absolute stack after add |
| added_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-CreateTable"></a>

### CreateTable



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_name | [string](#string) |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | [int64](#int64) |  |  |
| big_blind | [int64](#int64) |  |  |
| min_buy_in | [int64](#int64) |  |  |
| max_buy_in | [int64](#int64) |  |  |
| max_players | [int32](#int32) |  | 2-10 |
| action_timeout_seconds | [int32](#int32) |  |  |






<a name="examples-EndHand"></a>

### EndHand



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | [bytes](#bytes) |  |  |
| results | [PotResult](#examples-PotResult) | repeated |  |






<a name="examples-HandEnded"></a>

### HandEnded



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | [bytes](#bytes) |  |  |
| results | [PotResult](#examples-PotResult) | repeated |  |
| stack_changes | [HandEnded.StackChangesEntry](#examples-HandEnded-StackChangesEntry) | repeated | player_root_hex -&gt; delta |
| ended_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-HandEnded-StackChangesEntry"></a>

### HandEnded.StackChangesEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [string](#string) |  |  |
| value | [int64](#int64) |  |  |






<a name="examples-HandStarted"></a>

### HandStarted



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | [bytes](#bytes) |  | New hand aggregate root |
| hand_number | [int64](#int64) |  |  |
| dealer_position | [int32](#int32) |  |  |
| small_blind_position | [int32](#int32) |  |  |
| big_blind_position | [int32](#int32) |  |  |
| active_players | [SeatSnapshot](#examples-SeatSnapshot) | repeated |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | [int64](#int64) |  |  |
| big_blind | [int64](#int64) |  |  |
| started_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-JoinTable"></a>

### JoinTable



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| preferred_seat | [int32](#int32) |  | -1 for any available |
| buy_in_amount | [int64](#int64) |  |  |






<a name="examples-LeaveTable"></a>

### LeaveTable



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |






<a name="examples-PlayerJoined"></a>

### PlayerJoined



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| seat_position | [int32](#int32) |  |  |
| buy_in_amount | [int64](#int64) |  |  |
| stack | [int64](#int64) |  | Absolute stack after buy-in |
| joined_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PlayerLeft"></a>

### PlayerLeft



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| seat_position | [int32](#int32) |  |  |
| chips_cashed_out | [int64](#int64) |  |  |
| left_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PlayerSatIn"></a>

### PlayerSatIn



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| sat_in_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PlayerSatOut"></a>

### PlayerSatOut



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |
| sat_out_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-PotResult"></a>

### PotResult



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| winner_root | [bytes](#bytes) |  |  |
| amount | [int64](#int64) |  |  |
| pot_type | [string](#string) |  | &#34;main&#34; or &#34;side_N&#34; |
| winning_hand | [HandRanking](#examples-HandRanking) |  |  |






<a name="examples-SeatSnapshot"></a>

### SeatSnapshot



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| position | [int32](#int32) |  |  |
| player_root | [bytes](#bytes) |  |  |
| stack | [int64](#int64) |  |  |






<a name="examples-SitIn"></a>

### SitIn



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |






<a name="examples-SitOut"></a>

### SitOut



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | [bytes](#bytes) |  |  |






<a name="examples-StartHand"></a>

### StartHand
No parameters - uses current table state
Dealer button advances automatically






<a name="examples-TableCreated"></a>

### TableCreated



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_name | [string](#string) |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | [int64](#int64) |  |  |
| big_blind | [int64](#int64) |  |  |
| min_buy_in | [int64](#int64) |  |  |
| max_buy_in | [int64](#int64) |  |  |
| max_players | [int32](#int32) |  |  |
| action_timeout_seconds | [int32](#int32) |  |  |
| created_at | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="examples-TableState"></a>

### TableState
State (for snapshots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_id | [string](#string) |  |  |
| table_name | [string](#string) |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | [int64](#int64) |  |  |
| big_blind | [int64](#int64) |  |  |
| min_buy_in | [int64](#int64) |  |  |
| max_buy_in | [int64](#int64) |  |  |
| max_players | [int32](#int32) |  |  |
| action_timeout_seconds | [int32](#int32) |  |  |
| seats | [Seat](#examples-Seat) | repeated |  |
| dealer_position | [int32](#int32) |  |  |
| hand_count | [int64](#int64) |  |  |
| current_hand_root | [bytes](#bytes) |  |  |
| status | [string](#string) |  | &#34;waiting&#34;, &#34;in_hand&#34;, &#34;paused&#34; |





 

 

 

 



<a name="io_cloudevents_v1_cloudevents-proto"></a>
<p align="right"><a href="#top">Top</a></p>

## io/cloudevents/v1/cloudevents.proto



<a name="io-cloudevents-v1-CloudEvent"></a>

### CloudEvent
CloudEvent represents a single CloudEvent in protobuf format.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | [string](#string) |  | Required Attributes |
| source | [string](#string) |  | URI-reference |
| spec_version | [string](#string) |  |  |
| type | [string](#string) |  |  |
| attributes | [CloudEvent.AttributesEntry](#io-cloudevents-v1-CloudEvent-AttributesEntry) | repeated | Optional &amp; Extension Attributes |
| binary_data | [bytes](#bytes) |  | Binary data |
| text_data | [string](#string) |  | Text data |
| proto_data | [google.protobuf.Any](#google-protobuf-Any) |  | Protobuf message |






<a name="io-cloudevents-v1-CloudEvent-AttributesEntry"></a>

### CloudEvent.AttributesEntry



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | [string](#string) |  |  |
| value | [CloudEventAttributeValue](#io-cloudevents-v1-CloudEventAttributeValue) |  |  |






<a name="io-cloudevents-v1-CloudEventAttributeValue"></a>

### CloudEventAttributeValue
CloudEventAttributeValue supports the CloudEvents type system.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| ce_boolean | [bool](#bool) |  |  |
| ce_integer | [int32](#int32) |  |  |
| ce_string | [string](#string) |  |  |
| ce_bytes | [bytes](#bytes) |  |  |
| ce_uri | [string](#string) |  |  |
| ce_uri_ref | [string](#string) |  |  |
| ce_timestamp | [google.protobuf.Timestamp](#google-protobuf-Timestamp) |  |  |






<a name="io-cloudevents-v1-CloudEventBatch"></a>

### CloudEventBatch
CloudEventBatch is a container for multiple CloudEvents.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [CloudEvent](#io-cloudevents-v1-CloudEvent) | repeated |  |





 

 

 

 



## Scalar Value Types

| .proto Type | Notes | C++ | Java | Python | Go | C# | PHP | Ruby |
| ----------- | ----- | --- | ---- | ------ | -- | -- | --- | ---- |
| <a name="double" /> double |  | double | double | float | float64 | double | float | Float |
| <a name="float" /> float |  | float | float | float | float32 | float | float | Float |
| <a name="int32" /> int32 | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint32 instead. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="int64" /> int64 | Uses variable-length encoding. Inefficient for encoding negative numbers – if your field is likely to have negative values, use sint64 instead. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="uint32" /> uint32 | Uses variable-length encoding. | uint32 | int | int/long | uint32 | uint | integer | Bignum or Fixnum (as required) |
| <a name="uint64" /> uint64 | Uses variable-length encoding. | uint64 | long | int/long | uint64 | ulong | integer/string | Bignum or Fixnum (as required) |
| <a name="sint32" /> sint32 | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int32s. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="sint64" /> sint64 | Uses variable-length encoding. Signed int value. These more efficiently encode negative numbers than regular int64s. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="fixed32" /> fixed32 | Always four bytes. More efficient than uint32 if values are often greater than 2^28. | uint32 | int | int | uint32 | uint | integer | Bignum or Fixnum (as required) |
| <a name="fixed64" /> fixed64 | Always eight bytes. More efficient than uint64 if values are often greater than 2^56. | uint64 | long | int/long | uint64 | ulong | integer/string | Bignum |
| <a name="sfixed32" /> sfixed32 | Always four bytes. | int32 | int | int | int32 | int | integer | Bignum or Fixnum (as required) |
| <a name="sfixed64" /> sfixed64 | Always eight bytes. | int64 | long | int/long | int64 | long | integer/string | Bignum |
| <a name="bool" /> bool |  | bool | boolean | boolean | bool | bool | boolean | TrueClass/FalseClass |
| <a name="string" /> string | A string must always contain UTF-8 encoded or 7-bit ASCII text. | string | String | str/unicode | string | string | string | String (UTF-8) |
| <a name="bytes" /> bytes | May contain any arbitrary sequence of bytes. | string | ByteString | str | []byte | ByteString | string | String (ASCII-8BIT) |

