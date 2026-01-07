# Future Enhancements

## Cloud Deployments

### AWS (EKS + MemoryDB)
- EKS cluster with managed node groups
- MemoryDB for Redis (event store)
- RDS Aurora Serverless (optional relational data)
- SQS/SNS for event bus
- ALB with gRPC support
- IAM roles for service accounts (IRSA)

### GCP (GKE + Bigtable)
- GKE Autopilot or Standard cluster
- Bigtable for event store (wide-column, high-scale)
- Pub/Sub for event bus
- Cloud Load Balancing with gRPC
- Workload Identity for GCP service access

## Storage Implementations

### BigtableEventStore
Row key design:
- `{domain}#{root}#{sequence:010d}` - Padded sequence for lexicographic ordering
- Column family: `events` with columns for event data
- Append-only with conditional mutations for optimistic locking

### DynamoDB EventStore  
- Partition key: `{domain}#{root}`
- Sort key: `sequence`
- Streams for change data capture

## Event Bus Alternatives

### Kafka EventBus
- Topic per domain or all events topic
- Consumer groups for projectors
- Exactly-once semantics via transactions

### AWS SNS/SQS
- SNS topic for fan-out
- SQS queues per consumer
- Dead letter queues for failed processing

### Google Pub/Sub
- Topic per domain
- Push subscriptions for low latency
- Pull subscriptions for batch processing

## Observability

### Metrics (Prometheus)
- Request latency histograms
- Event throughput counters
- Storage operation durations
- gRPC method statistics

### Tracing (OpenTelemetry)
- Distributed tracing across services
- Trace context propagation
- Span attributes for events

### Dashboards
- Grafana dashboards for operations
- Alert rules for SLOs
