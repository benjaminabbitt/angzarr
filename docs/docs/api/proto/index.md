---
title: Protocol Buffer API
description: Auto-generated documentation for Angzarr protobuf definitions
---

# Protocol Documentation

## Table of Contents {#top}

- [angzarr/aggregate.proto](#angzarr_aggregate-proto)
    - [BusinessResponse](#angzarr-BusinessResponse)
    - [CommandResponse](#angzarr-CommandResponse)
    - [ReplayRequest](#angzarr-ReplayRequest)
    - [ReplayResponse](#angzarr-ReplayResponse)
    - [RevocationResponse](#angzarr-RevocationResponse)
    - [SpeculateAggregateRequest](#angzarr-SpeculateAggregateRequest)

    - [CommandHandlerCoordinatorService](#angzarr-CommandHandlerCoordinatorService)
    - [CommandHandlerService](#angzarr-CommandHandlerService)
  
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



<p align="right"><a href="#top">Top</a></p>

## angzarr/aggregate.proto {#angzarr_aggregate-proto}




### BusinessResponse {#angzarr-BusinessResponse}
Wrapper response for BusinessLogic.Handle


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  | Business provides compensation events |
| revocation | [RevocationResponse](#angzarr-RevocationResponse) |  | Business requests framework action |
| notification | [Notification](#angzarr-Notification) |  | Forward rejection notification upstream |







### CommandResponse {#angzarr-CommandResponse}
Response from entity - aggregate events &#43; sync projector results


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  | Events from the target aggregate |
| projections | [Projection](#angzarr-Projection) | repeated | Synchronous projector results |







### ReplayRequest {#angzarr-ReplayRequest}
Request to replay events and compute resulting state


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| base_snapshot | [Snapshot](#angzarr-Snapshot) |  | Starting state (empty = initial state) |
| events | [EventPage](#angzarr-EventPage) | repeated | Events to apply in order |







### ReplayResponse {#angzarr-ReplayResponse}
Response with computed state after replay


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| state | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  | Resulting state |







### RevocationResponse {#angzarr-RevocationResponse}
client logic requests framework to handle revocation


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| emit_system_revocation | bool |  | Emit SagaCompensationFailed event |
| send_to_dead_letter_queue | bool |  | Send to DLQ |
| escalate | bool |  | Flag for alerting/human intervention |
| abort | bool |  | Stop saga chain, propagate error to caller |
| reason | string |  | Context/reason |







### SpeculateAggregateRequest {#angzarr-SpeculateAggregateRequest}
Request for speculative command execution against temporal state.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| command | [CommandBook](#angzarr-CommandBook) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 



### CommandHandlerCoordinatorService {#angzarr-CommandHandlerCoordinatorService}
CommandHandlerCoordinatorService: orchestrates command processing for aggregates

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Handle | [CommandBook](#angzarr-CommandBook) | [CommandResponse](#angzarr-CommandResponse) | Async processing - fire and forget |
| HandleSync | [SyncCommandBook](#angzarr-SyncCommandBook) | [CommandResponse](#angzarr-CommandResponse) | Sync processing - waits for completion based on sync_mode |
| HandleSyncSpeculative | [SpeculateAggregateRequest](#angzarr-SpeculateAggregateRequest) | [CommandResponse](#angzarr-CommandResponse) | Speculative execution - execute against temporal state without persisting |
| HandleCompensation | [CommandBook](#angzarr-CommandBook) | [BusinessResponse](#angzarr-BusinessResponse) | Compensation flow - returns BusinessResponse for saga compensation handling. If business returns events, persists them. Caller handles revocation flags. |



### CommandHandlerService {#angzarr-CommandHandlerService}
CommandHandlerService: client logic that processes commands and emits events
client logic doesn&#39;t care about sync - coordinator decides

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Handle | [ContextualCommand](#angzarr-ContextualCommand) | [BusinessResponse](#angzarr-BusinessResponse) | Process command and return business response (events or revocation request) |
| Replay | [ReplayRequest](#angzarr-ReplayRequest) | [ReplayResponse](#angzarr-ReplayResponse) | Replay events to compute state (for conflict detection) Optional: only needed if aggregate supports MERGE_COMMUTATIVE |

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/cloudevents.proto {#angzarr_cloudevents-proto}




### CloudEvent {#angzarr-CloudEvent}
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
| type | string |  | Event type (e.g., &#34;com.example.order.created&#34;). Default: proto type_url suffix from original event. |
| data | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  | Event payload as proto Any. Framework converts to JSON for CloudEvents output. Client should filter sensitive fields before packing. |
| extensions | [CloudEvent.ExtensionsEntry](#angzarr-CloudEvent-ExtensionsEntry) | repeated | Custom extension attributes. Keys should follow CloudEvents naming (lowercase, no dots). Framework adds correlationid automatically if present in Cover. |
| id | string | optional | Optional overrides. Framework uses Cover/EventPage values if not set.

Default: `{domain}`:`{root_id}`:`{sequence}` |
| source | string | optional | Default: angzarr/`{domain}` |
| subject | string | optional | Default: aggregate root ID |







### CloudEvent.ExtensionsEntry {#angzarr-CloudEvent-ExtensionsEntry}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | string |  |  |
| value | string |  |  |







### CloudEventsResponse {#angzarr-CloudEventsResponse}
CloudEventsResponse is returned by client projectors in Projection.projection.

Framework detects this type by checking projection.type_url and routes
the events to configured sinks (HTTP webhook, Kafka).

Client may return 0 events (skip), 1 event (typical), or N events
(fan-out scenarios like multi-tenant notifications).


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [CloudEvent](#angzarr-CloudEvent) | repeated |  |





 

 

 

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/meta.proto {#angzarr_meta-proto}




### DeleteEditionEvents {#angzarr-DeleteEditionEvents}
Delete all events for an edition&#43;domain combination.
Main timeline (&#39;angzarr&#39; or empty edition name) cannot be deleted.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| edition | string |  | Edition name to delete from |
| domain | string |  | Domain to delete from |







### EditionEventsDeleted {#angzarr-EditionEventsDeleted}
Response from edition event deletion.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| edition | string |  |  |
| domain | string |  |  |
| deleted_count | uint32 |  |  |
| deleted_at | string |  |  |





 

 

 

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/process_manager.proto {#angzarr_process_manager-proto}




### ProcessManagerHandleRequest {#angzarr-ProcessManagerHandleRequest}
Phase 2 request: full context for PM decision.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| trigger | [EventBook](#angzarr-EventBook) |  | Full state of triggering domain. |
| process_state | [EventBook](#angzarr-EventBook) |  | Current process manager state (event-sourced). |
| destinations | [EventBook](#angzarr-EventBook) | repeated | Additional destinations fetched per Prepare response. |







### ProcessManagerHandleResponse {#angzarr-ProcessManagerHandleResponse}
Phase 2 response: commands and PM events.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| commands | [CommandBook](#angzarr-CommandBook) | repeated | Commands to issue to other aggregates. |
| process_events | [EventBook](#angzarr-EventBook) |  | Events for the process manager&#39;s own domain (non-duplicative workflow state). These are persisted via CommandHandlerCoordinator to the PM&#39;s domain. |







### ProcessManagerPrepareRequest {#angzarr-ProcessManagerPrepareRequest}
Phase 1 request: PM declares additional destinations needed.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| trigger | [EventBook](#angzarr-EventBook) |  | Full state of triggering domain (by correlation_id). |
| process_state | [EventBook](#angzarr-EventBook) |  | Current process manager state (by correlation_id). May be empty for new workflow. |







### ProcessManagerPrepareResponse {#angzarr-ProcessManagerPrepareResponse}
Phase 1 response: destinations to fetch before Handle.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| destinations | [Cover](#angzarr-Cover) | repeated | Additional aggregates needed beyond trigger. Query by correlation_id. Minimize fetches - only declare what&#39;s actually needed. |







### SpeculatePmRequest {#angzarr-SpeculatePmRequest}
Request for speculative PM execution at a point in time.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| request | [ProcessManagerHandleRequest](#angzarr-ProcessManagerHandleRequest) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 



### ProcessManagerCoordinatorService {#angzarr-ProcessManagerCoordinatorService}
ProcessManagerCoordinatorService: orchestrates PM execution

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| HandleSpeculative | [SpeculatePmRequest](#angzarr-SpeculatePmRequest) | [ProcessManagerHandleResponse](#angzarr-ProcessManagerHandleResponse) | Speculative execution - returns commands and events without persisting |



### ProcessManagerService {#angzarr-ProcessManagerService}
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
It reuses all aggregate infrastructure (EventStore, SnapshotStore, CommandHandlerCoordinator).

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Prepare | [ProcessManagerPrepareRequest](#angzarr-ProcessManagerPrepareRequest) | [ProcessManagerPrepareResponse](#angzarr-ProcessManagerPrepareResponse) | Phase 1: Declare which additional destinations are needed beyond the trigger. PM automatically receives triggering event&#39;s domain state. |
| Handle | [ProcessManagerHandleRequest](#angzarr-ProcessManagerHandleRequest) | [ProcessManagerHandleResponse](#angzarr-ProcessManagerHandleResponse) | Phase 2: Handle with trigger &#43; process state &#43; fetched destinations. Returns commands for other aggregates and events for the PM&#39;s own domain. |

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/projector.proto {#angzarr_projector-proto}




### SpeculateProjectorRequest {#angzarr-SpeculateProjectorRequest}
Request for speculative projector execution at a point in time.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 



### ProjectorCoordinatorService {#angzarr-ProjectorCoordinatorService}
ProjectorCoordinatorService: orchestrates projection processing

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| HandleSync | [SyncEventBook](#angzarr-SyncEventBook) | [Projection](#angzarr-Projection) | Sync processing - returns projection based on sync_mode |
| Handle | [EventBook](#angzarr-EventBook) | [.google.protobuf.Empty](https://protobuf.dev/reference/protobuf/google.protobuf/#empty) | Async processing - fire and forget |
| HandleSpeculative | [SpeculateProjectorRequest](#angzarr-SpeculateProjectorRequest) | [Projection](#angzarr-Projection) | Speculative processing - returns projection without side effects |



### ProjectorService {#angzarr-ProjectorService}
ProjectorService: client logic that projects events to read models
client logic doesn&#39;t care about sync - coordinator decides

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Handle | [EventBook](#angzarr-EventBook) | [Projection](#angzarr-Projection) | Async projection - projector should persist and return |
| HandleSpeculative | [EventBook](#angzarr-EventBook) | [Projection](#angzarr-Projection) | Speculative processing - projector must avoid external side effects |

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/query.proto {#angzarr_query-proto}


 

 

 



### EventQueryService {#angzarr-EventQueryService}
EventQueryService: query interface for retrieving events

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| GetEventBook | [Query](#angzarr-Query) | [EventBook](#angzarr-EventBook) | Get a single EventBook (unary) - use for explicit queries with gRPC tooling |
| GetEvents | [Query](#angzarr-Query) | [EventBook](#angzarr-EventBook) stream | Stream EventBooks matching query - use for bulk retrieval |
| Synchronize | [Query](#angzarr-Query) stream | [EventBook](#angzarr-EventBook) stream |  |
| GetAggregateRoots | [.google.protobuf.Empty](https://protobuf.dev/reference/protobuf/google.protobuf/#empty) | [AggregateRoot](#angzarr-AggregateRoot) stream |  |

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/saga.proto {#angzarr_saga-proto}




### SagaCompensationFailed {#angzarr-SagaCompensationFailed}
System event when compensation fails/requested


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| triggering_aggregate | [Cover](#angzarr-Cover) |  |  |
| triggering_event_sequence | uint32 |  |  |
| saga_name | string |  |  |
| rejection_reason | string |  |  |
| compensation_failure_reason | string |  |  |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  |  |
| occurred_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### SagaExecuteRequest {#angzarr-SagaExecuteRequest}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [EventBook](#angzarr-EventBook) |  | Source events (same as prepare) |
| destinations | [EventBook](#angzarr-EventBook) | repeated | Fetched destination state |







### SagaPrepareRequest {#angzarr-SagaPrepareRequest}
Two-phase saga protocol messages


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [EventBook](#angzarr-EventBook) |  | Source events that triggered the saga |







### SagaPrepareResponse {#angzarr-SagaPrepareResponse}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| destinations | [Cover](#angzarr-Cover) | repeated | Destination aggregates the saga needs to read |







### SagaResponse {#angzarr-SagaResponse}
Response from saga - commands for other aggregates


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| commands | [CommandBook](#angzarr-CommandBook) | repeated | Commands to execute on other aggregates |
| events | [EventBook](#angzarr-EventBook) | repeated | Events to publish directly |







### SagaRetryRequest {#angzarr-SagaRetryRequest}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| source | [EventBook](#angzarr-EventBook) |  |  |
| destinations | [EventBook](#angzarr-EventBook) | repeated |  |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  |  |
| rejection_reason | string |  |  |
| attempt | uint32 |  |  |







### SpeculateSagaRequest {#angzarr-SpeculateSagaRequest}
Request for speculative saga execution at a point in time.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| request | [SagaExecuteRequest](#angzarr-SagaExecuteRequest) |  |  |
| point_in_time | [TemporalQuery](#angzarr-TemporalQuery) |  |  |





 

 

 



### SagaCoordinatorService {#angzarr-SagaCoordinatorService}
SagaCoordinatorService: orchestrates saga execution

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Execute | [SagaExecuteRequest](#angzarr-SagaExecuteRequest) | [SagaResponse](#angzarr-SagaResponse) | Async processing - fire and forget |
| ExecuteSpeculative | [SpeculateSagaRequest](#angzarr-SpeculateSagaRequest) | [SagaResponse](#angzarr-SagaResponse) | Speculative execution - returns commands without side effects |



### SagaService {#angzarr-SagaService}
SagaService: client logic that coordinates across aggregates
Two-phase protocol: Prepare (declare destinations) → Execute (with fetched state)

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Prepare | [SagaPrepareRequest](#angzarr-SagaPrepareRequest) | [SagaPrepareResponse](#angzarr-SagaPrepareResponse) | Phase 1: Saga declares which destination aggregates it needs |
| Execute | [SagaExecuteRequest](#angzarr-SagaExecuteRequest) | [SagaResponse](#angzarr-SagaResponse) | Phase 2: Execute with source &#43; fetched destination state |

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/stream.proto {#angzarr_stream-proto}


 

 

 



### EventStreamService {#angzarr-EventStreamService}
docs:start:event_stream_service
EventStreamService: streams events to registered subscribers

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Subscribe | [EventStreamFilter](#angzarr-EventStreamFilter) | [EventBook](#angzarr-EventBook) stream | Subscribe to events matching correlation ID (required) Returns INVALID_ARGUMENT if correlation_id is empty |

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/types.proto {#angzarr_types-proto}




### AggregateRoot {#angzarr-AggregateRoot}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | string |  |  |
| root | [UUID](#angzarr-UUID) |  |  |







### AngzarrDeadLetter {#angzarr-AngzarrDeadLetter}
docs:start:dead_letter
Dead letter queue entry for failed messages requiring manual intervention.
Per-domain topics: angzarr.dlq.`{domain}`


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  | Routing: domain, root, correlation_id |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  | Command that failed |
| rejected_events | [EventBook](#angzarr-EventBook) |  | Events that failed (saga/projector failures) |
| rejection_reason | string |  | Human-readable reason |
| sequence_mismatch | [SequenceMismatchDetails](#angzarr-SequenceMismatchDetails) |  | Sequence conflict details |
| event_processing_failed | [EventProcessingFailedDetails](#angzarr-EventProcessingFailedDetails) |  | Handler failure details |
| payload_retrieval_failed | [PayloadRetrievalFailedDetails](#angzarr-PayloadRetrievalFailedDetails) |  | Payload store failure details |
| occurred_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |
| metadata | [AngzarrDeadLetter.MetadataEntry](#angzarr-AngzarrDeadLetter-MetadataEntry) | repeated | Additional context |
| source_component | string |  | Which component sent to DLQ |
| source_component_type | string |  | &#34;aggregate&#34; | &#34;saga&#34; | &#34;projector&#34; | &#34;process_manager&#34; |







### AngzarrDeadLetter.MetadataEntry {#angzarr-AngzarrDeadLetter-MetadataEntry}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | string |  |  |
| value | string |  |  |







### CommandBook {#angzarr-CommandBook}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  |  |
| pages | [CommandPage](#angzarr-CommandPage) | repeated |  |
| saga_origin | [SagaCommandOrigin](#angzarr-SagaCommandOrigin) |  | Field 3 removed: correlation_id moved to Cover

Tracks origin for compensation flow |







### CommandPage {#angzarr-CommandPage}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sequence | uint32 |  | Expected sequence number for this command&#39;s events. Must match the aggregate&#39;s current next sequence (i.e., events.len()). For new aggregates, use 0. |
| merge_strategy | [MergeStrategy](#angzarr-MergeStrategy) |  |  |
| command | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  |  |
| external | [PayloadReference](#angzarr-PayloadReference) |  | Claim check: payload stored externally |







### ComponentDescriptor {#angzarr-ComponentDescriptor}
Component self-description.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | string |  |  |
| component_type | string |  |  |
| inputs | [Target](#angzarr-Target) | repeated | Domains I subscribe to (event types I consume) |







### ContextualCommand {#angzarr-ContextualCommand}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  | Passed from aggregate coordinator to aggregate, consists of everything needed to execute/evaluate the command |
| command | [CommandBook](#angzarr-CommandBook) |  |  |







### Cover {#angzarr-Cover}
docs:start:cover


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | string |  |  |
| root | [UUID](#angzarr-UUID) |  |  |
| correlation_id | string |  | Workflow correlation - flows through all commands/events |
| edition | [Edition](#angzarr-Edition) |  | Edition for diverged timelines; empty name = main timeline |







### DomainDivergence {#angzarr-DomainDivergence}
Explicit divergence point for a specific domain.
Used when creating historical branches or coordinating saga writes across domains.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | string |  | Domain name |
| sequence | uint32 |  | Divergence sequence number |







### Edition {#angzarr-Edition}
docs:start:edition
Edition identifier with optional explicit divergence points.

Two modes:
- Implicit (divergences empty): Divergence derived from first edition event&#39;s sequence
- Explicit (divergences populated): Per-domain divergence points for historical branching,
 saga coordination, or speculative execution


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| name | string |  | Edition name, e.g., &#34;v2&#34;; empty = main timeline |
| divergences | [DomainDivergence](#angzarr-DomainDivergence) | repeated | Optional: explicit per-domain divergence points |







### EventBook {#angzarr-EventBook}
docs:start:event_book


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  |  |
| snapshot | [Snapshot](#angzarr-Snapshot) |  | Snapshot state; sequence computed by framework on persist |
| pages | [EventPage](#angzarr-EventPage) | repeated |  |
| next_sequence | uint32 |  | Field 4 removed: correlation_id moved to Cover Field 5 removed: snapshot_state unified into snapshot field

Computed on load, never stored: (last page seq OR snapshot seq if no pages) &#43; 1 |







### EventPage {#angzarr-EventPage}
docs:start:event_page


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sequence | uint32 |  |  |
| created_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |
| event | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  |  |
| external | [PayloadReference](#angzarr-PayloadReference) |  | Claim check: payload stored externally |







### EventProcessingFailedDetails {#angzarr-EventProcessingFailedDetails}
Event processing failure details for DLQ entries.
Contains information about why a saga/projector failed to process events.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| error | string |  | Error message from the handler |
| retry_count | uint32 |  | Number of retry attempts before DLQ routing |
| is_transient | bool |  | Whether the failure is considered transient |







### EventStreamFilter {#angzarr-EventStreamFilter}
docs:start:event_stream_filter
Subscription filter for event streaming


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| correlation_id | string |  |  |







### GetDescriptorRequest {#angzarr-GetDescriptorRequest}
Request for GetDescriptor RPC.







### Notification {#angzarr-Notification}
docs:start:notification
Base notification message for transient system signals.
Contains routing info via Cover but no persistence semantics.
Type discrimination via payload.type_url (standard Any behavior).


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  | Routing: domain, root, correlation_id |
| payload | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  | Type-specific content (RejectionNotification, etc.) |
| sent_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  | When notification was created |
| metadata | [Notification.MetadataEntry](#angzarr-Notification-MetadataEntry) | repeated | Optional key-value metadata |







### Notification.MetadataEntry {#angzarr-Notification-MetadataEntry}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | string |  |  |
| value | string |  |  |







### PayloadReference {#angzarr-PayloadReference}
Reference to externally stored payload (claim check pattern).
Used when event/command payloads exceed message bus size limits.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| storage_type | [PayloadStorageType](#angzarr-PayloadStorageType) |  |  |
| uri | string |  | Location URI: - file:///var/angzarr/payloads/`{hash}`.bin - gs://bucket/prefix/`{hash}`.bin - s3://bucket/prefix/`{hash}`.bin |
| content_hash | bytes |  | Content hash for integrity verification and deduplication (SHA-256) |
| original_size | uint64 |  | Original serialized payload size in bytes |
| stored_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  | Timestamp when payload was stored (for TTL cleanup) |







### PayloadRetrievalFailedDetails {#angzarr-PayloadRetrievalFailedDetails}
Payload retrieval failure details for DLQ entries.
Contains information about why an externally stored payload couldn&#39;t be retrieved.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| storage_type | [PayloadStorageType](#angzarr-PayloadStorageType) |  | Storage backend type |
| uri | string |  | URI of the payload that couldn&#39;t be retrieved |
| content_hash | bytes |  | Content hash for identification |
| original_size | uint64 |  | Original payload size in bytes |
| error | string |  | Error message from the retrieval attempt |







### Projection {#angzarr-Projection}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  |  |
| projector | string |  |  |
| sequence | uint32 |  |  |
| projection | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  |  |







### Query {#angzarr-Query}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cover | [Cover](#angzarr-Cover) |  | Cover identifies the aggregate: domain &#43; (root | correlation_id | both) Query by root: Cover `{ domain, root }` Query by correlation: Cover `{ domain, correlation_id }` |
| range | [SequenceRange](#angzarr-SequenceRange) |  |  |
| sequences | [SequenceSet](#angzarr-SequenceSet) |  |  |
| temporal | [TemporalQuery](#angzarr-TemporalQuery) |  |  |







### RejectionNotification {#angzarr-RejectionNotification}
docs:start:rejection_notification
Notification payload for command rejection scenarios.
Embedded in Notification.payload when a saga/PM command is rejected.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| rejected_command | [CommandBook](#angzarr-CommandBook) |  | The command that was rejected (full context) |
| rejection_reason | string |  | Why: &#34;insufficient_funds&#34;, &#34;out_of_stock&#34;, etc. |
| issuer_name | string |  | Saga/PM name that issued the command |
| issuer_type | string |  | &#34;saga&#34; | &#34;process_manager&#34; |
| source_aggregate | [Cover](#angzarr-Cover) |  | Aggregate that originally triggered the flow |
| source_event_sequence | uint32 |  | Event sequence that triggered the saga/PM |







### SagaCommandOrigin {#angzarr-SagaCommandOrigin}
Track saga command origin for compensation flow


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| saga_name | string |  | Name of the saga that issued the command |
| triggering_aggregate | [Cover](#angzarr-Cover) |  | Domain&#43;root of aggregate that triggered the saga |
| triggering_event_sequence | uint32 |  | Sequence number of the triggering event |







### SequenceMismatchDetails {#angzarr-SequenceMismatchDetails}
docs:start:dlq_details
Sequence mismatch details for DLQ entries.
Contains expected vs actual sequence for debugging and replay.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| expected_sequence | uint32 |  | What the command expected |
| actual_sequence | uint32 |  | What the aggregate was at |
| merge_strategy | [MergeStrategy](#angzarr-MergeStrategy) |  | Strategy that triggered DLQ routing |







### SequenceRange {#angzarr-SequenceRange}
Query types


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| lower | uint32 |  |  |
| upper | uint32 | optional | If not set, query to latest |







### SequenceSet {#angzarr-SequenceSet}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| values | uint32 | repeated |  |







### Snapshot {#angzarr-Snapshot}
docs:start:aggregate_snapshot
Snapshot of aggregate state at a given sequence number.
State must be a protobuf Message to serialize into Any.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| sequence | uint32 |  |  |
| state | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  |  |
| retention | [SnapshotRetention](#angzarr-SnapshotRetention) |  | Controls cleanup behavior |







### SyncCommandBook {#angzarr-SyncCommandBook}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| command | [CommandBook](#angzarr-CommandBook) |  |  |
| sync_mode | [SyncMode](#angzarr-SyncMode) |  |  |







### SyncContextualCommand {#angzarr-SyncContextualCommand}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| command | [ContextualCommand](#angzarr-ContextualCommand) |  |  |
| sync_mode | [SyncMode](#angzarr-SyncMode) |  |  |







### SyncEventBook {#angzarr-SyncEventBook}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventBook](#angzarr-EventBook) |  |  |
| sync_mode | [SyncMode](#angzarr-SyncMode) |  |  |







### Target {#angzarr-Target}
Describes what a component subscribes to.
Topology edges derived from inputs: if A subscribes to domain X, edge X→A exists.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | string |  |  |







### TemporalQuery {#angzarr-TemporalQuery}
Temporal query: retrieve aggregate state at a point in history.
Replays events from sequence 0 (no snapshots) to the specified point.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| as_of_time | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  | Events with created_at &lt;= this |
| as_of_sequence | uint32 |  | Events with sequence &lt;= this |







### UUID {#angzarr-UUID}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| value | bytes |  |  |





 



### MergeStrategy {#angzarr-MergeStrategy}
docs:start:merge_strategy
Controls how concurrent commands to the same aggregate are handled

| Name | Number | Description |
| ---- | ------ | ----------- |
| MERGE_COMMUTATIVE | 0 | Default: allow if state field mutations don&#39;t overlap |
| MERGE_STRICT | 1 | Reject if sequence mismatch (optimistic concurrency) |
| MERGE_AGGREGATE_HANDLES | 2 | Aggregate handles its own concurrency |
| MERGE_MANUAL | 3 | Send to DLQ for manual review on mismatch |




### PayloadStorageType {#angzarr-PayloadStorageType}
docs:start:payload_reference
Storage backend type for externally stored payloads (claim check pattern).

| Name | Number | Description |
| ---- | ------ | ----------- |
| PAYLOAD_STORAGE_TYPE_UNSPECIFIED | 0 |  |
| PAYLOAD_STORAGE_TYPE_FILESYSTEM | 1 |  |
| PAYLOAD_STORAGE_TYPE_GCS | 2 |  |
| PAYLOAD_STORAGE_TYPE_S3 | 3 |  |




### SnapshotRetention {#angzarr-SnapshotRetention}
docs:start:snapshot_retention
Controls snapshot retention during cleanup

| Name | Number | Description |
| ---- | ------ | ----------- |
| RETENTION_DEFAULT | 0 | Persist every 16 events, treated as TRANSIENT otherwise |
| RETENTION_PERSIST | 1 | Keep indefinitely (business milestone) |
| RETENTION_TRANSIENT | 2 | Delete when newer snapshot written |




### SyncMode {#angzarr-SyncMode}
docs:start:sync_mode
Controls synchronous processing behavior

| Name | Number | Description |
| ---- | ------ | ----------- |
| SYNC_MODE_UNSPECIFIED | 0 | Async: fire and forget (default) |
| SYNC_MODE_SIMPLE | 1 | Sync projectors only, no saga cascade |
| SYNC_MODE_CASCADE | 2 | Full sync: projectors &#43; saga cascade (expensive) |


 

 

 



<p align="right"><a href="#top">Top</a></p>

## angzarr/upcaster.proto {#angzarr_upcaster-proto}




### UpcastRequest {#angzarr-UpcastRequest}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| domain | string |  |  |
| events | [EventPage](#angzarr-EventPage) | repeated |  |







### UpcastResponse {#angzarr-UpcastResponse}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| events | [EventPage](#angzarr-EventPage) | repeated |  |





 

 

 



### UpcasterService {#angzarr-UpcasterService}
UpcasterService: transforms old event versions to current versions
Implemented by the client alongside CommandHandlerService on the same gRPC server.
Optionally can be deployed as a separate binary for testing or complex migrations.

| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| Upcast | [UpcastRequest](#angzarr-UpcastRequest) | [UpcastResponse](#angzarr-UpcastResponse) | Transform events to current version Returns events in same order, transformed where applicable |

 



<p align="right"><a href="#top">Top</a></p>

## examples/ai_sidecar.proto {#examples_ai_sidecar-proto}




### ActionHistory {#examples-ActionHistory}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| action | [ActionType](#examples-ActionType) |  |  |
| amount | int64 |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |







### ActionRequest {#examples-ActionRequest}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| model_id | string |  | Game state

Which model to use |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |
| hole_cards | [Card](#examples-Card) | repeated | Cards |
| community_cards | [Card](#examples-Card) | repeated |  |
| pot_size | int64 |  | Betting context |
| stack_size | int64 |  |  |
| amount_to_call | int64 |  |  |
| min_raise | int64 |  |  |
| max_raise | int64 |  |  |
| position | int32 |  | Position info

0 = button, increasing = earlier |
| players_remaining | int32 |  |  |
| players_to_act | int32 |  |  |
| action_history | [ActionHistory](#examples-ActionHistory) | repeated | Historical context (for recurrent models) |
| opponents | [OpponentStats](#examples-OpponentStats) | repeated | Opponent modeling (optional) |







### ActionResponse {#examples-ActionResponse}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| recommended_action | [ActionType](#examples-ActionType) |  |  |
| amount | int64 |  | For bet/raise |
| fold_probability | float |  | Confidence scores for each action (for analysis) |
| check_call_probability | float |  |  |
| bet_raise_probability | float |  |  |
| model_version | string |  | Model metadata |
| inference_time_ms | int64 |  |  |







### BatchActionRequest {#examples-BatchActionRequest}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| requests | [ActionRequest](#examples-ActionRequest) | repeated |  |







### BatchActionResponse {#examples-BatchActionResponse}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| responses | [ActionResponse](#examples-ActionResponse) | repeated |  |







### HealthRequest {#examples-HealthRequest}








### HealthResponse {#examples-HealthResponse}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| healthy | bool |  |  |
| model_id | string |  |  |
| model_version | string |  |  |
| uptime_seconds | int64 |  |  |
| requests_served | int64 |  |  |







### OpponentStats {#examples-OpponentStats}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| position | int32 |  |  |
| stack | int64 |  |  |
| vpip | float |  | Voluntarily put in pot % |
| pfr | float |  | Pre-flop raise % |
| aggression | float |  | Bet/raise frequency |
| hands_played | int32 |  |  |





 

 

 



### AiSidecar {#examples-AiSidecar}


| Method Name | Request Type | Response Type | Description |
| ----------- | ------------ | ------------- | ------------|
| GetAction | [ActionRequest](#examples-ActionRequest) | [ActionResponse](#examples-ActionResponse) | Get recommended action from the AI model |
| Health | [HealthRequest](#examples-HealthRequest) | [HealthResponse](#examples-HealthResponse) | Health check |
| GetActionsBatch | [BatchActionRequest](#examples-BatchActionRequest) | [BatchActionResponse](#examples-BatchActionResponse) | Batch inference for training/simulation |

 



<p align="right"><a href="#top">Top</a></p>

## examples/hand.proto {#examples_hand-proto}




### ActionTaken {#examples-ActionTaken}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| action | [ActionType](#examples-ActionType) |  |  |
| amount | int64 |  |  |
| player_stack | int64 |  | Absolute stack after action |
| pot_total | int64 |  | Absolute pot after action |
| amount_to_call | int64 |  | Current call amount for next player |
| action_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### AwardPot {#examples-AwardPot}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| awards | [PotAward](#examples-PotAward) | repeated |  |







### BettingRoundComplete {#examples-BettingRoundComplete}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| completed_phase | [BettingPhase](#examples-BettingPhase) |  |  |
| pot_total | int64 |  |  |
| stacks | [PlayerStackSnapshot](#examples-PlayerStackSnapshot) | repeated |  |
| completed_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### BlindPosted {#examples-BlindPosted}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| blind_type | string |  |  |
| amount | int64 |  |  |
| player_stack | int64 |  | Absolute stack after posting |
| pot_total | int64 |  | Absolute pot after posting |
| posted_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### CardsDealt {#examples-CardsDealt}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | bytes |  |  |
| hand_number | int64 |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| player_cards | [PlayerHoleCards](#examples-PlayerHoleCards) | repeated |  |
| dealer_position | int32 |  |  |
| players | [PlayerInHand](#examples-PlayerInHand) | repeated |  |
| dealt_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |
| remaining_deck | [Card](#examples-Card) | repeated | Cards left after dealing hole cards |







### CardsMucked {#examples-CardsMucked}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| mucked_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### CardsRevealed {#examples-CardsRevealed}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| cards | [Card](#examples-Card) | repeated |  |
| ranking | [HandRanking](#examples-HandRanking) |  |  |
| revealed_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### CommunityCardsDealt {#examples-CommunityCardsDealt}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| cards | [Card](#examples-Card) | repeated |  |
| phase | [BettingPhase](#examples-BettingPhase) |  | FLOP, TURN, or RIVER |
| all_community_cards | [Card](#examples-Card) | repeated | Full board so far |
| dealt_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### DealCards {#examples-DealCards}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | bytes |  |  |
| hand_number | int64 |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| players | [PlayerInHand](#examples-PlayerInHand) | repeated |  |
| dealer_position | int32 |  |  |
| small_blind | int64 |  |  |
| big_blind | int64 |  |  |
| deck_seed | bytes |  | For deterministic shuffle (testing) |







### DealCommunityCards {#examples-DealCommunityCards}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| count | int32 |  | 3 for flop, 1 for turn/river |







### DrawCompleted {#examples-DrawCompleted}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| cards_discarded | int32 |  |  |
| cards_drawn | int32 |  |  |
| new_cards | [Card](#examples-Card) | repeated | Only visible to this player |
| drawn_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### HandComplete {#examples-HandComplete}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | bytes |  |  |
| hand_number | int64 |  |  |
| winners | [PotWinner](#examples-PotWinner) | repeated |  |
| final_stacks | [PlayerStackSnapshot](#examples-PlayerStackSnapshot) | repeated |  |
| completed_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### HandState {#examples-HandState}
State (for snapshots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_id | string |  |  |
| table_root | bytes |  |  |
| hand_number | int64 |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| remaining_deck | [Card](#examples-Card) | repeated | Deck state |
| players | [PlayerHandState](#examples-PlayerHandState) | repeated | Player state |
| community_cards | [Card](#examples-Card) | repeated | Community cards |
| current_phase | [BettingPhase](#examples-BettingPhase) |  | Betting state |
| action_on_position | int32 |  |  |
| current_bet | int64 |  |  |
| min_raise | int64 |  |  |
| pots | [Pot](#examples-Pot) | repeated |  |
| dealer_position | int32 |  | Positions |
| small_blind_position | int32 |  |  |
| big_blind_position | int32 |  |  |
| status | string |  | &#34;dealing&#34;, &#34;betting&#34;, &#34;showdown&#34;, &#34;complete&#34; |







### PlayerAction {#examples-PlayerAction}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| action | [ActionType](#examples-ActionType) |  |  |
| amount | int64 |  | For bet/raise/call |







### PlayerHandState {#examples-PlayerHandState}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| position | int32 |  |  |
| hole_cards | [Card](#examples-Card) | repeated |  |
| stack | int64 |  |  |
| bet_this_round | int64 |  |  |
| total_invested | int64 |  |  |
| has_acted | bool |  |  |
| has_folded | bool |  |  |
| is_all_in | bool |  |  |







### PlayerHoleCards {#examples-PlayerHoleCards}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| cards | [Card](#examples-Card) | repeated |  |







### PlayerInHand {#examples-PlayerInHand}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| position | int32 |  |  |
| stack | int64 |  |  |







### PlayerStackSnapshot {#examples-PlayerStackSnapshot}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| stack | int64 |  |  |
| is_all_in | bool |  |  |
| has_folded | bool |  |  |







### PlayerTimedOut {#examples-PlayerTimedOut}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| default_action | [ActionType](#examples-ActionType) |  | Usually FOLD or CHECK |
| timed_out_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PostBlind {#examples-PostBlind}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| blind_type | string |  | &#34;small&#34;, &#34;big&#34;, &#34;ante&#34; |
| amount | int64 |  |  |







### PotAward {#examples-PotAward}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| amount | int64 |  |  |
| pot_type | string |  |  |







### PotAwarded {#examples-PotAwarded}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| winners | [PotWinner](#examples-PotWinner) | repeated |  |
| awarded_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PotWinner {#examples-PotWinner}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| amount | int64 |  |  |
| pot_type | string |  |  |
| winning_hand | [HandRanking](#examples-HandRanking) |  |  |







### RequestDraw {#examples-RequestDraw}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| card_indices | int32 | repeated | Which cards to discard (0-indexed) |







### RevealCards {#examples-RevealCards}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| muck | bool |  | True to hide cards (fold at showdown) |







### ShowdownStarted {#examples-ShowdownStarted}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| players_to_show | bytes | repeated | Order of revelation |
| started_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |





 

 

 

 



<p align="right"><a href="#top">Top</a></p>

## examples/player.proto {#examples_player-proto}




### ActionRequested {#examples-ActionRequested}
Emitted when action is needed - AI players respond via sidecar


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | bytes |  |  |
| table_root | bytes |  |  |
| player_root | bytes |  |  |
| player_type | [PlayerType](#examples-PlayerType) |  |  |
| amount_to_call | int64 |  |  |
| min_raise | int64 |  |  |
| max_raise | int64 |  |  |
| hole_cards | [Card](#examples-Card) | repeated |  |
| community_cards | [Card](#examples-Card) | repeated |  |
| pot_size | int64 |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |
| deadline | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### DepositFunds {#examples-DepositFunds}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |







### FundsDeposited {#examples-FundsDeposited}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| new_balance | [Currency](#examples-Currency) |  | Absolute value after deposit |
| deposited_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### FundsReleased {#examples-FundsReleased}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| table_root | bytes |  |  |
| new_available_balance | [Currency](#examples-Currency) |  |  |
| new_reserved_balance | [Currency](#examples-Currency) |  |  |
| released_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### FundsReserved {#examples-FundsReserved}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| table_root | bytes |  |  |
| new_available_balance | [Currency](#examples-Currency) |  | Bankroll minus reserved |
| new_reserved_balance | [Currency](#examples-Currency) |  | Total reserved across tables |
| reserved_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### FundsTransferred {#examples-FundsTransferred}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| from_player_root | bytes |  |  |
| to_player_root | bytes |  |  |
| amount | [Currency](#examples-Currency) |  |  |
| hand_root | bytes |  |  |
| reason | string |  |  |
| new_balance | [Currency](#examples-Currency) |  | Recipient&#39;s new balance |
| transferred_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### FundsWithdrawn {#examples-FundsWithdrawn}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| new_balance | [Currency](#examples-Currency) |  | Absolute value after withdrawal |
| withdrawn_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PlayerRegistered {#examples-PlayerRegistered}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| display_name | string |  |  |
| email | string |  |  |
| player_type | [PlayerType](#examples-PlayerType) |  |  |
| ai_model_id | string |  |  |
| registered_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PlayerState {#examples-PlayerState}
State (for snapshots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_id | string |  |  |
| display_name | string |  |  |
| email | string |  |  |
| player_type | [PlayerType](#examples-PlayerType) |  |  |
| ai_model_id | string |  |  |
| bankroll | [Currency](#examples-Currency) |  |  |
| reserved_funds | [Currency](#examples-Currency) |  |  |
| table_reservations | [PlayerState.TableReservationsEntry](#examples-PlayerState-TableReservationsEntry) | repeated | table_root_hex -&gt; amount |
| status | string |  | &#34;active&#34;, &#34;suspended&#34;, etc. |







### PlayerState.TableReservationsEntry {#examples-PlayerState-TableReservationsEntry}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | string |  |  |
| value | int64 |  |  |







### RegisterPlayer {#examples-RegisterPlayer}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| display_name | string |  |  |
| email | string |  | Used for root derivation |
| player_type | [PlayerType](#examples-PlayerType) |  | HUMAN or AI |
| ai_model_id | string |  | For AI players: which model to use |







### ReleaseFunds {#examples-ReleaseFunds}
Release reserved funds back to bankroll (leave table)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_root | bytes |  |  |







### RequestAction {#examples-RequestAction}
Request action from player (triggers AI sidecar for AI players)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | bytes |  |  |
| table_root | bytes |  |  |
| amount_to_call | int64 |  |  |
| min_raise | int64 |  |  |
| max_raise | int64 |  | Player&#39;s remaining stack |
| hole_cards | [Card](#examples-Card) | repeated |  |
| community_cards | [Card](#examples-Card) | repeated |  |
| pot_size | int64 |  |  |
| phase | [BettingPhase](#examples-BettingPhase) |  |  |
| timeout_seconds | int32 |  |  |







### ReserveFunds {#examples-ReserveFunds}
Reserve funds when joining a table (buy-in)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |
| table_root | bytes |  | Which table the funds are reserved for |







### TransferFunds {#examples-TransferFunds}
Transfer funds from one player to another (pot award)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| from_player_root | bytes |  | Source player (for reserved funds) |
| amount | [Currency](#examples-Currency) |  |  |
| hand_root | bytes |  | Which hand this transfer is for |
| reason | string |  | &#34;pot_win&#34;, &#34;side_pot_win&#34;, etc. |







### WithdrawFunds {#examples-WithdrawFunds}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | [Currency](#examples-Currency) |  |  |





 

 

 

 



<p align="right"><a href="#top">Top</a></p>

## examples/poker_types.proto {#examples_poker_types-proto}




### Card {#examples-Card}
Card representation


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| suit | [Suit](#examples-Suit) |  |  |
| rank | [Rank](#examples-Rank) |  |  |







### Currency {#examples-Currency}
Currency amount (in smallest unit, e.g., cents)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | int64 |  |  |
| currency_code | string |  | &#34;USD&#34;, &#34;EUR&#34;, &#34;CHIPS&#34; |







### HandRanking {#examples-HandRanking}
Hand ranking result


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| rank_type | [HandRankType](#examples-HandRankType) |  |  |
| kickers | [Rank](#examples-Rank) | repeated | For tie-breaking |
| score | int32 |  | Numeric score for comparison |







### Pot {#examples-Pot}
Pot structure (for side pots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| amount | int64 |  |  |
| eligible_players | bytes | repeated | Player roots eligible for this pot |
| pot_type | string |  | &#34;main&#34; or &#34;side_N&#34; |







### Seat {#examples-Seat}
Position at table


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| position | int32 |  | 0-9 for 10-max table |
| player_root | bytes |  | Player aggregate root |
| stack | [Currency](#examples-Currency) |  | Current stack at table |
| is_active | bool |  | Still in current hand |
| is_sitting_out | bool |  | Temporarily away |





 



### ActionType {#examples-ActionType}
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




### BettingPhase {#examples-BettingPhase}
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




### GameVariant {#examples-GameVariant}
Game variant configuration

| Name | Number | Description |
| ---- | ------ | ----------- |
| GAME_VARIANT_UNSPECIFIED | 0 |  |
| TEXAS_HOLDEM | 1 |  |
| OMAHA | 2 |  |
| FIVE_CARD_DRAW | 3 |  |
| SEVEN_CARD_STUD | 4 |  |




### HandRankType {#examples-HandRankType}


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




### PlayerType {#examples-PlayerType}
Player type - abstraction for human vs AI

| Name | Number | Description |
| ---- | ------ | ----------- |
| PLAYER_TYPE_UNSPECIFIED | 0 |  |
| HUMAN | 1 |  |
| AI | 2 |  |




### Rank {#examples-Rank}


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




### Suit {#examples-Suit}


| Name | Number | Description |
| ---- | ------ | ----------- |
| SUIT_UNSPECIFIED | 0 |  |
| CLUBS | 1 |  |
| DIAMONDS | 2 |  |
| HEARTS | 3 |  |
| SPADES | 4 |  |


 

 

 



<p align="right"><a href="#top">Top</a></p>

## examples/table.proto {#examples_table-proto}




### AddChips {#examples-AddChips}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| amount | int64 |  |  |







### ChipsAdded {#examples-ChipsAdded}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| amount | int64 |  |  |
| new_stack | int64 |  | Absolute stack after add |
| added_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### CreateTable {#examples-CreateTable}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_name | string |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | int64 |  |  |
| big_blind | int64 |  |  |
| min_buy_in | int64 |  |  |
| max_buy_in | int64 |  |  |
| max_players | int32 |  | 2-10 |
| action_timeout_seconds | int32 |  |  |







### EndHand {#examples-EndHand}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | bytes |  |  |
| results | [PotResult](#examples-PotResult) | repeated |  |







### HandEnded {#examples-HandEnded}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | bytes |  |  |
| results | [PotResult](#examples-PotResult) | repeated |  |
| stack_changes | [HandEnded.StackChangesEntry](#examples-HandEnded-StackChangesEntry) | repeated | player_root_hex -&gt; delta |
| ended_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### HandEnded.StackChangesEntry {#examples-HandEnded-StackChangesEntry}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | string |  |  |
| value | int64 |  |  |







### HandStarted {#examples-HandStarted}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| hand_root | bytes |  | New hand aggregate root |
| hand_number | int64 |  |  |
| dealer_position | int32 |  |  |
| small_blind_position | int32 |  |  |
| big_blind_position | int32 |  |  |
| active_players | [SeatSnapshot](#examples-SeatSnapshot) | repeated |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | int64 |  |  |
| big_blind | int64 |  |  |
| started_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### JoinTable {#examples-JoinTable}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| preferred_seat | int32 |  | -1 for any available |
| buy_in_amount | int64 |  |  |







### LeaveTable {#examples-LeaveTable}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |







### PlayerJoined {#examples-PlayerJoined}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| seat_position | int32 |  |  |
| buy_in_amount | int64 |  |  |
| stack | int64 |  | Absolute stack after buy-in |
| joined_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PlayerLeft {#examples-PlayerLeft}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| seat_position | int32 |  |  |
| chips_cashed_out | int64 |  |  |
| left_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PlayerSatIn {#examples-PlayerSatIn}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| sat_in_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PlayerSatOut {#examples-PlayerSatOut}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |
| sat_out_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### PotResult {#examples-PotResult}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| winner_root | bytes |  |  |
| amount | int64 |  |  |
| pot_type | string |  | &#34;main&#34; or &#34;side_N&#34; |
| winning_hand | [HandRanking](#examples-HandRanking) |  |  |







### SeatSnapshot {#examples-SeatSnapshot}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| position | int32 |  |  |
| player_root | bytes |  |  |
| stack | int64 |  |  |







### SitIn {#examples-SitIn}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |







### SitOut {#examples-SitOut}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| player_root | bytes |  |  |







### StartHand {#examples-StartHand}
No parameters - uses current table state
Dealer button advances automatically







### TableCreated {#examples-TableCreated}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_name | string |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | int64 |  |  |
| big_blind | int64 |  |  |
| min_buy_in | int64 |  |  |
| max_buy_in | int64 |  |  |
| max_players | int32 |  |  |
| action_timeout_seconds | int32 |  |  |
| created_at | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### TableState {#examples-TableState}
State (for snapshots)


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| table_id | string |  |  |
| table_name | string |  |  |
| game_variant | [GameVariant](#examples-GameVariant) |  |  |
| small_blind | int64 |  |  |
| big_blind | int64 |  |  |
| min_buy_in | int64 |  |  |
| max_buy_in | int64 |  |  |
| max_players | int32 |  |  |
| action_timeout_seconds | int32 |  |  |
| seats | [Seat](#examples-Seat) | repeated |  |
| dealer_position | int32 |  |  |
| hand_count | int64 |  |  |
| current_hand_root | bytes |  |  |
| status | string |  | &#34;waiting&#34;, &#34;in_hand&#34;, &#34;paused&#34; |





 

 

 

 



<p align="right"><a href="#top">Top</a></p>

## io/cloudevents/v1/cloudevents.proto {#io_cloudevents_v1_cloudevents-proto}




### CloudEvent {#io-cloudevents-v1-CloudEvent}
CloudEvent represents a single CloudEvent in protobuf format.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| id | string |  | Required Attributes |
| source | string |  | URI-reference |
| spec_version | string |  |  |
| type | string |  |  |
| attributes | [CloudEvent.AttributesEntry](#io-cloudevents-v1-CloudEvent-AttributesEntry) | repeated | Optional &amp; Extension Attributes |
| binary_data | bytes |  | Binary data |
| text_data | string |  | Text data |
| proto_data | [google.protobuf.Any](https://protobuf.dev/reference/protobuf/google.protobuf/#any) |  | Protobuf message |







### CloudEvent.AttributesEntry {#io-cloudevents-v1-CloudEvent-AttributesEntry}



| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| key | string |  |  |
| value | [CloudEventAttributeValue](#io-cloudevents-v1-CloudEventAttributeValue) |  |  |







### CloudEventAttributeValue {#io-cloudevents-v1-CloudEventAttributeValue}
CloudEventAttributeValue supports the CloudEvents type system.


| Field | Type | Label | Description |
| ----- | ---- | ----- | ----------- |
| ce_boolean | bool |  |  |
| ce_integer | int32 |  |  |
| ce_string | string |  |  |
| ce_bytes | bytes |  |  |
| ce_uri | string |  |  |
| ce_uri_ref | string |  |  |
| ce_timestamp | [google.protobuf.Timestamp](https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp) |  |  |







### CloudEventBatch {#io-cloudevents-v1-CloudEventBatch}
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

