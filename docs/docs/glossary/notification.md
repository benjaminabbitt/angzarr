---
id: notification
title: Notification
hoverText: Transient (non-persisted) system signal for compensation, rejections, or real-time alerts.
---

# Notification

Transient (non-persisted) system signals used for compensation signals, rejections, and real-time alerts.

## Event vs Notification

| Aspect | Event | Notification |
|--------|-------|--------------|
| Persisted | Yes (event store) | No (transient) |
| Sequenced | Yes | No |
| Replay | Can be replayed | Lost after delivery |
| Use case | State changes | System signals |

## Types of Notifications

### RejectionNotification
Sent when a saga/PM command is rejected by the target aggregate. Contains:
- Rejected command context
- Rejection reason
- Saga origin (for compensation routing)

### Compensation Signals
Sent to trigger compensation flows when downstream operations fail.

## When to Use

Use **Events** for:
- Business state changes that need audit trail
- Facts that might need replay
- Cross-domain communication via sagas

Use **Notifications** for:
- Error signals that trigger compensation
- Real-time alerts that don't need persistence
- System-level signals (health, metrics)
