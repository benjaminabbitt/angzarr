---
sidebar_position: 5
---

# DynamoDB

Amazon DynamoDB provides **serverless scaling** for event sourcing on AWS with pay-per-request pricing.

---

## Why DynamoDB

| Strength | Benefit |
|----------|---------|
| **Serverless** | No capacity planning |
| **Pay-per-use** | Cost scales with traffic |
| **Single-digit ms** | Consistent low latency |
| **AWS native** | Integrates with Lambda, Kinesis |
| **Global tables** | Multi-region replication |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **Cost at scale** | Can exceed provisioned alternatives |
| **AWS lock-in** | Not portable to other clouds |
| **25 KB item limit** | Large events need chunking |

---

## Configuration

```toml
[storage]
backend = "dynamo"

[storage.dynamo]
region = "us-east-1"
table_prefix = "angzarr"
# Optional: endpoint for local development
# endpoint = "http://localhost:8000"
```

### Environment Variables

```bash
export AWS_REGION="us-east-1"
export DYNAMO_TABLE_PREFIX="angzarr"
export STORAGE_BACKEND="dynamo"
# AWS credentials via standard mechanisms
```

---

## Table Schema

### Events Table

```
Table: angzarr-events
Partition Key: pk (String) = "{domain}#{edition}#{root}"
Sort Key: sk (Number) = sequence

Attributes:
  - event_type: String
  - payload: Binary
  - correlation_id: String (optional)
  - created_at: String (ISO 8601)
```

### Positions Table

```
Table: angzarr-positions
Partition Key: pk (String) = "{handler}#{domain}#{edition}#{root}"

Attributes:
  - sequence: Number
  - updated_at: String
```

### Snapshots Table

```
Table: angzarr-snapshots
Partition Key: pk (String) = "{domain}#{edition}#{root}"

Attributes:
  - sequence: Number
  - state: Binary
  - created_at: String
```

---

## Optimistic Concurrency

DynamoDB condition expressions enforce sequence ordering:

```python
# Conditional put - fails if sequence exists
dynamodb.put_item(
    TableName="angzarr-events",
    Item={"pk": pk, "sk": sequence, ...},
    ConditionExpression="attribute_not_exists(sk)"
)
```

---

## Local Development

Use DynamoDB Local for development:

```bash
# Start DynamoDB Local
docker run -p 8000:8000 amazon/dynamodb-local

# Configure endpoint
export DYNAMO_ENDPOINT="http://localhost:8000"
```

---

## Table Creation

```bash
# Events table
aws dynamodb create-table \
  --table-name angzarr-events \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
    AttributeName=sk,AttributeType=N \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
    AttributeName=sk,KeyType=RANGE \
  --billing-mode PAY_PER_REQUEST

# Positions table
aws dynamodb create-table \
  --table-name angzarr-positions \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST

# Snapshots table
aws dynamodb create-table \
  --table-name angzarr-snapshots \
  --attribute-definitions \
    AttributeName=pk,AttributeType=S \
  --key-schema \
    AttributeName=pk,KeyType=HASH \
  --billing-mode PAY_PER_REQUEST
```

---

## When to Use DynamoDB

- **AWS native** — Already on AWS
- **Variable load** — Traffic spikes/lulls
- **Serverless** — Lambda-based architecture
- **Global** — Multi-region requirements

---

## Next Steps

- **[Bigtable](/tooling/databases/bigtable)** — GCP equivalent
- **[PostgreSQL](/tooling/databases/postgres)** — Self-managed alternative
