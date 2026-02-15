# Saga Revocation

When a saga command is rejected by a target aggregate, angzarr initiates a **revocation flow** to handle compensation. This document details how the system detects failures, notifies the originating aggregate, and processes compensation decisions.

> For context on why saga commands should rarely fail, see [Transactional Guarantees](sagas.md#transactional-guarantees).

---

## Overview

```
1. Saga emits command → Target aggregate rejects
                            ↓
2. Framework detects rejection (command has saga_origin)
                            ↓
3. RevokeEventCommand sent to triggering aggregate
                            ↓
4. Business logic decides response (BusinessResponse)
                            ↓
5. Framework processes RevocationResponse flags
```

The key insight: when a saga command fails, the **triggering aggregate** (which emitted the event that started the saga) is notified and can emit compensation events.

---

## Protocol Messages

### RevokeEventCommand

Sent to the triggering aggregate when a saga command is rejected:

```protobuf
message RevokeEventCommand {
  uint32 triggering_event_sequence = 1;  // Which event triggered the saga
  string saga_name = 2;                   // Which saga failed
  string rejection_reason = 3;           // Why the command was rejected
  CommandBook rejected_command = 4;      // The command that failed
}
```

**File:** [`proto/angzarr/saga.proto`](../../../proto/angzarr/saga.proto)

The `triggering_event_sequence` identifies which event in the triggering aggregate's history caused the saga to run. This allows the aggregate to correlate the failure with its own state.

### BusinessResponse

The triggering aggregate's response to a `RevokeEventCommand`:

```protobuf
message BusinessResponse {
  oneof result {
    EventBook events = 1;               // Aggregate provides compensation events
    RevocationResponse revocation = 2;  // Request framework action
  }
}
```

**File:** [`proto/angzarr/aggregate.proto`](../../../proto/angzarr/aggregate.proto)

Two response modes:
1. **Events** — Aggregate handles compensation itself by emitting events
2. **RevocationResponse** — Aggregate requests framework-level actions

### RevocationResponse

Flags requesting framework action:

```protobuf
message RevocationResponse {
  bool emit_system_revocation = 1;      // Emit SagaCompensationFailed event
  bool send_to_dead_letter_queue = 2;   // Send to DLQ for manual review
  bool escalate = 3;                    // Trigger alerting/human intervention
  bool abort = 4;                       // Stop saga chain, propagate error
  string reason = 5;                    // Context for logging/debugging
}
```

Flags are processed in order and can be combined.

### SagaCompensationFailed

System event emitted when compensation cannot be handled by business logic:

```protobuf
message SagaCompensationFailed {
  Cover triggering_aggregate = 1;
  uint32 triggering_event_sequence = 2;
  string saga_name = 3;
  string rejection_reason = 4;
  string compensation_failure_reason = 5;
  CommandBook rejected_command = 6;
  google.protobuf.Timestamp occurred_at = 7;
}
```

**File:** [`proto/angzarr/saga.proto`](../../../proto/angzarr/saga.proto)

Emitted to a **fallback domain** (configurable, default: `angzarr.saga-failures`). Monitor this domain for systemic issues.

---

## Compensation Logic

**File:** [`src/utils/saga_compensation/mod.rs`](../../../src/utils/saga_compensation/mod.rs)

### CompensationContext

Built when a saga command is rejected:

```rust
pub struct CompensationContext {
    pub saga_origin: SagaCommandOrigin,   // From rejected command
    pub rejection_reason: String,          // Why rejected
    pub rejected_command: CommandBook,     // The failed command
    pub correlation_id: String,            // For tracing
}
```

Created via `CompensationContext::from_rejected_command()`. Returns `None` if the command lacks `saga_origin` (not a saga-issued command).

### Building the Revoke Command

```rust
pub fn build_revoke_command_book(context: &CompensationContext) -> Result<CommandBook>
```

Creates a `CommandBook` targeting the **triggering aggregate** (from `saga_origin.triggering_aggregate`). The command contains a serialized `RevokeEventCommand`.

### Handling Business Response

```rust
pub fn handle_business_response(
    response: Result<BusinessResponse, Status>,
    context: &CompensationContext,
    config: &SagaCompensationConfig,
) -> Result<CompensationOutcome>
```

Decision tree:

| Response | Action |
|----------|--------|
| `BusinessResponse::Events` with pages | Use compensation events |
| `BusinessResponse::Revocation` | Process flags |
| Empty response | Use fallback config |
| gRPC error | Use fallback config |

### CompensationOutcome

```rust
pub enum CompensationOutcome {
    Events(EventBook),              // Business provided compensation events
    EmitSystemRevocation(EventBook), // Emit SagaCompensationFailed to fallback domain
    Declined { reason: String },    // Just log, no action
    Aborted { reason: String },     // Stop saga chain, propagate error
}
```

### Flag Processing

Flags are processed in order:

```rust
fn process_revocation_flags(revocation, context, config) -> Result<CompensationOutcome>
```

1. **`send_to_dead_letter_queue`** — Send context to DLQ for manual review
2. **`escalate`** — Log at ERROR level, call webhook if configured
3. **`abort`** — Return error immediately, stop saga chain
4. **`emit_system_revocation`** — Emit `SagaCompensationFailed` to fallback domain

If no flags are set, returns `Declined` (just logs).

---

## Configuration

```rust
pub struct SagaCompensationConfig {
    pub fallback_domain: String,           // Where to emit SagaCompensationFailed
    pub fallback_emit_system_revocation: bool,
    pub fallback_send_to_dlq: bool,
    pub fallback_escalate: bool,
    pub dead_letter_queue_url: Option<String>,
    pub escalation_webhook_url: Option<String>,
}
```

**Fallback flags** are used when business logic returns an empty response or gRPC error. This ensures failures are never silently dropped.

### Environment Variables

```bash
ANGZARR_SAGA_FALLBACK_DOMAIN=angzarr.saga-failures
ANGZARR_SAGA_FALLBACK_EMIT_SYSTEM_REVOCATION=true
ANGZARR_SAGA_FALLBACK_SEND_TO_DLQ=false
ANGZARR_SAGA_FALLBACK_ESCALATE=true
ANGZARR_SAGA_DLQ_URL=amqp://localhost/saga-dlq
ANGZARR_SAGA_ESCALATION_WEBHOOK=https://alerts.example.com/saga-failure
```

---

## Complete Flow

```
Saga emits AddLoyaltyPoints → Customer aggregate rejects (customer not found)
                                          ↓
     Framework detects: command.saga_origin is Some
                                          ↓
     build_revoke_command_book(context)
                                          ↓
     RevokeEventCommand sent to Transaction aggregate (triggering_aggregate)
                                          ↓
     Transaction.Handle(RevokeEventCommand) → BusinessResponse
                                          ↓
     ┌────────────────────────────────────┼────────────────────────────────────┐
     ↓                                    ↓                                    ↓
  Events provided               RevocationResponse                    Empty/Error
  → Use compensation               → Process flags                    → Use fallback
     events                                                              config
                                          ↓
     ┌─────────────────┬─────────────────┬─────────────────┬─────────────────┐
     ↓                 ↓                 ↓                 ↓                 ↓
  emit_system      send_to_dlq       escalate           abort           (none)
  _revocation
     ↓                 ↓                 ↓                 ↓                 ↓
SagaCompensation   DLQ send         Log ERROR        Return error      Declined
Failed event       + manual         + webhook        (stop chain)      (just log)
to fallback        review
domain
```

---

## Implementing Compensation in Aggregates

When an aggregate receives a `RevokeEventCommand`, it should:

1. **Identify the triggering event** using `triggering_event_sequence`
2. **Decide on compensation** based on current state and rejection reason
3. **Return appropriate response**

### Python Client Helpers

The `angzarr_client` library provides helpers for common compensation patterns:

```python
from angzarr_client import Aggregate, handles
from angzarr_client.compensation import (
    CompensationContext,
    delegate_to_framework,
    emit_compensation_events,
)

class OrderAggregate(Aggregate[OrderState]):
    def handle_revocation(self, cmd):
        # Extract compensation context for easy access
        ctx = CompensationContext.from_revoke_command(cmd)

        # Option 1: Emit compensation events
        if ctx.saga_name == "saga-order-fulfillment":
            event = OrderCancelled(
                order_id=self.order_id,
                reason=f"Fulfillment failed: {ctx.rejection_reason}",
            )
            self._apply_and_record(event)
            return emit_compensation_events(self.event_book())

        # Option 2: Delegate to framework
        return delegate_to_framework(
            reason=f"No custom compensation for {ctx.saga_name}"
        )
```

**Helpers:**
- `CompensationContext.from_revoke_command(cmd)` — Extract fields into a convenient dataclass
- `delegate_to_framework(reason, emit_system_event=True, ...)` — Build `RevocationResponse`
- `emit_compensation_events(event_book)` — Build `BusinessResponse` with events

### Go Client Helpers

The `angzarr` Go package provides helpers for common compensation patterns:

```go
import angzarr "github.com/benjaminabbitt/angzarr/client/go"

func handleRevocation(cmd *pb.RevokeEventCommand, state OrderState) *pb.BusinessResponse {
    ctx := angzarr.NewCompensationContext(cmd)

    // Option 1: Emit compensation events
    if ctx.SagaName == "saga-order-fulfillment" {
        event := &OrderCancelled{
            OrderId: state.OrderId,
            Reason:  fmt.Sprintf("Fulfillment failed: %s", ctx.RejectionReason),
        }
        return angzarr.EmitCompensationEvents(packEvents(event))
    }

    // Option 2: Delegate to framework
    return angzarr.DelegateToFramework("No custom compensation for " + ctx.SagaName)
}

// Register with router
router := angzarr.NewCommandRouter("order", rebuildState).
    On("CreateOrder", handleCreateOrder).
    WithRevocationHandler(handleRevocation)
```

**Helpers:**
- `NewCompensationContext(cmd)` — Extract fields into a convenient struct
- `DelegateToFramework(reason)` — Build `RevocationResponse`
- `EmitCompensationEvents(events)` — Build `BusinessResponse` with events

### Rust: Compensation Events

```rust
fn handle_revocation(&self, cmd: &RevokeEventCommand) -> BusinessResponse {
    // Emit compensation event
    let compensation = TransactionReversed {
        original_sequence: cmd.triggering_event_sequence,
        reason: format!("Saga {} failed: {}", cmd.saga_name, cmd.rejection_reason),
    };

    BusinessResponse {
        result: Some(business_response::Result::Events(EventBook {
            pages: vec![pack_event(compensation)],
            ..Default::default()
        })),
    }
}
```

### Rust: Request Framework Action

```rust
fn handle_revocation(&self, cmd: &RevokeEventCommand) -> BusinessResponse {
    // Cannot handle this compensation - escalate
    BusinessResponse {
        result: Some(business_response::Result::Revocation(RevocationResponse {
            emit_system_revocation: true,
            escalate: true,
            reason: "Manual review required for this failure type".into(),
            ..Default::default()
        })),
    }
}
```

---

## Process Manager Compensation

Process Managers are also aggregates and can receive revocation requests when their commands are rejected. PMs have additional considerations:

1. **PM state tracking** — PMs may want to record failures in their own event-sourced state
2. **Multi-domain correlation** — PMs coordinate across domains and may need to track which steps failed

### Python PM Compensation

```python
from angzarr_client import ProcessManager, reacts_to
from angzarr_client.compensation import (
    CompensationContext,
    pm_delegate_to_framework,
    pm_emit_compensation_events,
)

class OrderWorkflowPM(ProcessManager[WorkflowState]):
    def handle_revocation(self, cmd):
        ctx = CompensationContext.from_revoke_command(cmd)

        # Record failure in PM state
        event = WorkflowStepFailed(
            saga_name=ctx.saga_name,
            reason=ctx.rejection_reason,
            failed_at=now(),
        )
        self._apply_and_record(event)

        # Return PM events + optionally emit system event
        return pm_emit_compensation_events(
            process_events=self.process_events(),
            also_emit_system_event=True,
            reason=f"PM recorded failure for {ctx.saga_name}",
        )
```

**PM Helpers:**
- `pm_delegate_to_framework(reason)` — Returns `(None, RevocationResponse)`
- `pm_emit_compensation_events(events, also_emit_system_event)` — Returns `(EventBook, RevocationResponse)`

### Go PM Compensation

```go
import angzarr "github.com/benjaminabbitt/angzarr/client/go"

func handlePMRevocation(cmd *pb.RevokeEventCommand, processState *pb.EventBook) *angzarr.PMRevocationResponse {
    ctx := angzarr.NewCompensationContext(cmd)

    // Record failure in PM state
    event := &WorkflowStepFailed{
        SagaName: ctx.SagaName,
        Reason:   ctx.RejectionReason,
    }
    events := packEvents(event)

    // Return PM events + emit system event
    return angzarr.PMEmitCompensationEvents(
        events,
        true, // also emit system event
        fmt.Sprintf("PM recorded failure for %s", ctx.SagaName),
    )
}

// Register with PM handler
handler := angzarr.NewProcessManagerHandler("order-workflow").
    ListenTo("order", "OrderCreated").
    WithHandle(handlePMEvents).
    WithRevocationHandler(handlePMRevocation)
```

**Go PM Helpers:**
- `PMDelegateToFramework(reason)` — Returns `*PMRevocationResponse` with no events
- `PMEmitCompensationEvents(events, emitSystemEvent, reason)` — Returns `*PMRevocationResponse` with events

### Rust PM Compensation

```rust
fn handle_revocation(
    &self,
    cmd: &RevokeEventCommand,
    process_state: Option<&EventBook>,
) -> (Option<EventBook>, RevocationResponse) {
    // Record failure in PM state
    let event = WorkflowStepFailed {
        saga_name: cmd.saga_name.clone(),
        reason: cmd.rejection_reason.clone(),
    };
    let events = pack_events(vec![event]);

    // Return PM events + framework response
    (
        Some(events),
        RevocationResponse {
            emit_system_revocation: true,
            reason: format!("PM recorded failure for saga {}", cmd.saga_name),
            ..Default::default()
        },
    )
}
```

The PM's `handle_revocation` returns a tuple:
- `Option<EventBook>` — PM events to persist (records failure in PM state)
- `RevocationResponse` — Framework action flags

---

## Monitoring

### Fallback Domain

Subscribe to the fallback domain (`angzarr.saga-failures` by default) to detect compensation failures:

```rust
// Projector subscribed to saga-failures domain
async fn handle(&self, events: &EventBook) -> Result<(), Status> {
    for page in &events.pages {
        if let Some(failed) = unpack::<SagaCompensationFailed>(&page.event) {
            metrics::saga_compensation_failed.inc();
            alert::send(format!(
                "Saga {} compensation failed for {}: {}",
                failed.saga_name,
                failed.triggering_aggregate.domain,
                failed.compensation_failure_reason
            ));
        }
    }
    Ok(())
}
```

### Metrics

Track:
- `saga_commands_rejected_total` — Commands rejected by target aggregates
- `saga_compensations_handled_total` — Successful business compensation
- `saga_compensations_failed_total` — Events emitted to fallback domain
- `saga_compensation_latency_seconds` — Time from rejection to resolution

### Alerting

Configure `escalation_webhook_url` to receive POST requests on escalation:

```json
{
  "saga_name": "saga-order-fulfillment",
  "triggering_aggregate": {
    "domain": "order",
    "root": "order-123"
  },
  "triggering_event_sequence": 5,
  "rejection_reason": "Inventory not available",
  "compensation_reason": "Business logic returned empty response",
  "occurred_at": "2025-01-15T10:30:00Z"
}
```

---

## Design Philosophy

### Revocation Is Exceptional

From [Transactional Guarantees](sagas.md#transactional-guarantees):

> **Saga commands should not fail under normal operation.**

Revocation handles:
- **Race conditions** — Customer deleted between event and saga command
- **External failures** — Payment gateway timeout
- **Bug recovery** — Logic errors discovered after events persisted
- **Manual intervention** — Business decision to reverse a workflow

If compensation happens frequently, it signals poor domain design. Invariants should be enforced in the triggering aggregate, not discovered at saga execution.

### Fallback Safety

The fallback configuration ensures failures are never silently dropped. Even if business logic crashes or returns garbage, the framework will:

1. Use configured fallback flags
2. Emit `SagaCompensationFailed` if enabled
3. Send to DLQ if configured
4. Trigger escalation if enabled

This guarantees visibility into compensation failures.

### Correlation ID Propagation

The `correlation_id` flows through the entire compensation chain:
- Original command → Triggering event → Saga command → Rejection → Revoke command → Compensation events

This enables end-to-end tracing of failures across domains.

---

## Related

- [Sagas](sagas.md) — Saga concepts and implementation
- [Compensation Flow](sagas.md#compensation-flow) — High-level compensation overview
- [Transactional Guarantees](sagas.md#transactional-guarantees) — Why saga commands should succeed
- [Reservation Pattern](sagas.md#reservation-pattern) — Expected releases vs error compensation
