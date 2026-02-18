---
sidebar_position: 5
---

# Payload Offloading

When event or command payloads exceed message bus size limits, angzarr stores them externally using the **claim check pattern**.

---

## Overview

```
Large Payload Flow:

1. Event with 5MB payload
2. Payload stored externally → returns PayloadReference
3. EventPage contains reference, not payload
4. Published to message bus (small message)
5. Consumer retrieves payload via reference
6. TTL reaper cleans up after retention period
```

The claim check pattern trades latency for reliability — payloads that would fail bus size limits are stored separately and retrieved on demand.

---

## When to Use

| Scenario | Solution |
|----------|----------|
| Event payload > 256KB (typical bus limit) | Payload offloading |
| Snapshot state > bus limit | Payload offloading |
| Binary attachments (images, documents) | Payload offloading |
| Normal-sized events | Direct embedding (no offloading) |

---

## Storage Backends

### Filesystem

Local storage for development and standalone mode:

```yaml
payload_offload:
  enabled: true
  store_type: filesystem
  filesystem:
    base_path: /var/angzarr/payloads
```

Files stored as: `/var/angzarr/payloads/{sha256-hash}.bin`

### Google Cloud Storage

For GCP deployments:

```yaml
payload_offload:
  enabled: true
  store_type: gcs
  gcs:
    bucket: my-angzarr-payloads
    prefix: events/  # Optional path prefix
```

Files stored as: `gs://my-angzarr-payloads/events/{sha256-hash}.bin`

### Amazon S3

For AWS deployments:

```yaml
payload_offload:
  enabled: true
  store_type: s3
  s3:
    bucket: my-angzarr-payloads
    prefix: events/
    region: us-east-1
    endpoint: http://localhost:4566  # Optional (LocalStack)
```

Files stored as: `s3://my-angzarr-payloads/events/{sha256-hash}.bin`

---

## Content-Addressable Storage

All backends use SHA-256 content hashing:

| Benefit | How |
|---------|-----|
| **Deduplication** | Identical payloads share storage |
| **Integrity** | Hash verified on retrieval |
| **Immutability** | Same hash = same content forever |

```
Payload: [binary data...]
Hash: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
URI: gs://bucket/e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.bin
```

---

## PayloadReference

Events and commands reference external payloads via `PayloadReference`:

```protobuf
message PayloadReference {
  PayloadStorageType storage_type = 1;  // FILESYSTEM | GCS | S3
  string uri = 2;                       // Full storage URI
  bytes content_hash = 3;               // SHA-256 for verification
  uint64 original_size = 4;             // Size in bytes
  Timestamp stored_at = 5;              // For TTL cleanup
}

message EventPage {
  // ... other fields ...
  google.protobuf.Any event = 4;                  // Empty when offloaded
  optional PayloadReference external_payload = 10; // Set when offloaded
}
```

---

## Threshold Configuration

Configure when offloading triggers:

```yaml
payload_offload:
  enabled: true
  threshold_bytes: 262144  # 256KB - offload payloads larger than this
  store_type: gcs
  gcs:
    bucket: my-payloads
```

---

## TTL and Cleanup

External payloads have a retention period. The `TtlReaper` background task cleans up expired payloads:

```yaml
payload_offload:
  ttl_days: 30              # Delete payloads older than this
  reaper_interval_hours: 24 # Run cleanup every 24 hours
```

### Cleanup Process

```
1. Reaper scans storage for payloads older than TTL
2. Cross-references with event store (are events still live?)
3. Deletes orphaned payloads
4. Logs cleanup metrics
```

### Manual Cleanup

For immediate cleanup:

```bash
# Using angzarr CLI
angzarr payload-store cleanup --older-than 7d

# Or via API
curl -X POST http://localhost:9099/admin/payload-store/cleanup?age=7d
```

---

## Retrieval Failures

When payload retrieval fails, angzarr routes to DLQ:

```protobuf
message PayloadRetrievalFailedDetails {
  PayloadStorageType storage_type = 1;
  string uri = 2;
  bytes content_hash = 3;
  uint64 original_size = 4;
  string error = 5;  // "Object not found", "Integrity check failed", etc.
}
```

Common failure causes:

| Error | Cause | Resolution |
|-------|-------|------------|
| Object not found | TTL expired, manual deletion | Restore from backup or skip |
| Integrity failed | Corruption, hash mismatch | Restore from backup |
| Access denied | Permissions changed | Fix IAM/bucket policies |
| Timeout | Network issues | Retry or check connectivity |

---

## Usage in Handlers

Payload offloading is transparent to handlers — the framework handles storage and retrieval:

```python
# Handler receives full payload regardless of storage location
def handle_large_document(state, cmd):
    # cmd.document_bytes is already retrieved
    # No special handling needed
    return DocumentUploaded(
        document_id=cmd.document_id,
        size=len(cmd.document_bytes),
        hash=compute_hash(cmd.document_bytes),
    )
```

### Manual Offloading (Advanced)

For explicit control:

```rust
use angzarr::payload_store::PayloadStore;

async fn store_large_payload(
    store: &dyn PayloadStore,
    data: &[u8],
) -> Result<PayloadReference, PayloadStoreError> {
    store.put(data).await
}

async fn retrieve_payload(
    store: &dyn PayloadStore,
    reference: &PayloadReference,
) -> Result<Vec<u8>, PayloadStoreError> {
    store.get(reference).await
}
```

---

## Monitoring

### Metrics

| Metric | Description |
|--------|-------------|
| `payload_store_put_total` | Total payloads stored |
| `payload_store_put_bytes_total` | Total bytes stored |
| `payload_store_get_total` | Total payloads retrieved |
| `payload_store_get_errors_total` | Retrieval failures |
| `payload_store_cleanup_deleted_total` | Payloads deleted by reaper |

### Alerts

```yaml
# Prometheus alerts
- alert: PayloadRetrievalErrors
  expr: rate(payload_store_get_errors_total[5m]) > 0.1
  labels:
    severity: warning

- alert: PayloadStorageGrowth
  expr: payload_store_put_bytes_total > 100e9  # 100GB
  labels:
    severity: info
```

---

## Best Practices

### 1. Set Appropriate Thresholds

Match your message bus limits:

| Bus | Typical Limit | Recommended Threshold |
|-----|---------------|----------------------|
| Kafka | 1MB default | 512KB |
| RabbitMQ | Unlimited* | 256KB |
| Pub/Sub | 10MB | 1MB |
| SNS/SQS | 256KB | 200KB |

### 2. Use Appropriate TTL

Balance storage costs vs replay needs:

| Use Case | Recommended TTL |
|----------|-----------------|
| Short-lived events | 7 days |
| Standard retention | 30 days |
| Compliance/audit | 365+ days |

### 3. Monitor Storage Growth

Payload stores can grow quickly with large events:

```bash
# Check storage size
gsutil du -s gs://my-payloads/
aws s3 ls s3://my-payloads/ --summarize
```

### 4. Handle Retrieval Failures Gracefully

DLQ entries for payload failures need manual intervention — the original payload may be unrecoverable.

---

## Next Steps

- **[Error Recovery](/operations/error-recovery)** — DLQ and retry handling
- **[Infrastructure](/operations/infrastructure)** — Deployment configuration
