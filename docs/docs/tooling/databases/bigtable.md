---
sidebar_position: 4
---

# Bigtable

Google Cloud Bigtable provides **petabyte-scale** storage for high-volume event sourcing on Google Cloud Platform.

---

## Why Bigtable

| Strength | Benefit |
|----------|---------|
| **Massive scale** | Petabytes, millions of ops/second |
| **Low latency** | Single-digit millisecond reads/writes |
| **Managed** | No operational overhead |
| **GCP native** | Integrates with Dataflow, BigQuery |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **Cost** | Pay for provisioned capacity |
| **GCP lock-in** | Not portable to other clouds |
| **Schema design** | Row key design is critical |

---

## Configuration

```toml
[storage]
backend = "bigtable"

[storage.bigtable]
project_id = "my-gcp-project"
instance_id = "angzarr-events"
```

### Environment Variables

```bash
export BIGTABLE_PROJECT_ID="my-gcp-project"
export BIGTABLE_INSTANCE_ID="angzarr-events"
export STORAGE_BACKEND="bigtable"
export GOOGLE_APPLICATION_CREDENTIALS="/path/to/service-account.json"
```

---

## Table Schema

Angzarr creates tables with this structure:

```
Table: events
Row key: {domain}#{edition}#{root}#{sequence_padded}
Column family: e
  - type: event type name
  - payload: serialized event
  - correlation_id: optional correlation
  - created_at: timestamp

Table: positions
Row key: {handler}#{domain}#{edition}#{root}
Column family: p
  - sequence: last processed sequence

Table: snapshots
Row key: {domain}#{edition}#{root}
Column family: s
  - sequence: snapshot sequence
  - state: serialized state
```

---

## Row Key Design

The row key format enables efficient queries:

```
# All events for an aggregate (prefix scan)
player#angzarr#abc123#*

# Events after sequence 100
player#angzarr#abc123#00000100 → player#angzarr#abc123#99999999
```

Sequence numbers are zero-padded for lexicographic ordering.

---

## Instance Setup

```bash
# Create Bigtable instance
gcloud bigtable instances create angzarr-events \
  --cluster=angzarr-c1 \
  --cluster-zone=us-central1-a \
  --cluster-num-nodes=3 \
  --display-name="Angzarr Events"

# Create tables
cbt -instance angzarr-events createtable events
cbt -instance angzarr-events createfamily events e

cbt -instance angzarr-events createtable positions
cbt -instance angzarr-events createfamily positions p

cbt -instance angzarr-events createtable snapshots
cbt -instance angzarr-events createfamily snapshots s
```

---

## When to Use Bigtable

- **High volume** — Millions of events per second
- **GCP native** — Already on Google Cloud
- **Analytics** — Feed events to BigQuery/Dataflow
- **Global** — Multi-region replication

---

## Next Steps

- **[DynamoDB](/tooling/databases/dynamo)** — AWS equivalent
- **[PostgreSQL](/tooling/databases/postgres)** — Simpler alternative
