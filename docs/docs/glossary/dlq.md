---
id: dlq
title: Dead Letter Queue (DLQ)
hoverText: Destination for messages that can't be processed automatically, requiring manual review.
---

# Dead Letter Queue (DLQ)

A destination for messages that cannot be processed automatically. Messages are routed to the DLQ when:

1. **Sequence mismatch** with `MERGE_MANUAL` strategy
2. **Processing failures** after retry exhaustion
3. **Payload retrieval failures** (external storage unavailable)
4. **Unrecoverable errors** in handlers

## DLQ Entry Types

### SequenceMismatchDetails
```protobuf
message SequenceMismatchDetails {
  uint64 expected_sequence = 1;
  uint64 actual_sequence = 2;
  MergeStrategy merge_strategy = 3;
}
```

### EventProcessingFailedDetails
```protobuf
message EventProcessingFailedDetails {
  string error_message = 1;
  uint32 retry_count = 2;
  bool is_transient = 3;
}
```

### PayloadRetrievalFailedDetails
For claim-check pattern failures when external payload cannot be retrieved.

## Topic Structure

Per-domain DLQ topics: `angzarr.dlq.{domain}`

## Resolution

DLQ messages require manual intervention:
1. Inspect the failed message and context
2. Fix the underlying issue
3. Resubmit or discard the message
4. Update monitoring/alerting as needed
