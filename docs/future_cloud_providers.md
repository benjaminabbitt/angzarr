# Serverless Deployment: Cloud Run & AWS Lambda

## Goal
Alternate deployment path for angzarr on serverless platforms (no K8s required).
- Cloud-native messaging: Pub/Sub (GCP), SQS/SNS/Kinesis (AWS)
- Simplified ops: No cluster management
- Feature-gated: `--features cloudrun` or `--features lambda`

## Architecture Change

**Current (K8s):** Long-running AMQP consumers with persistent broker connections, K8s label discovery

**Serverless:** HTTP-triggered handlers (Pub/Sub push / SQS Lambda triggers), environment URL discovery, ephemeral execution

```
[Client] -> [API Gateway] -> [Gateway Function]
                                    |
                                    v
                          [Aggregate Function] -> [Pub/Sub or SNS]
                                                        |
                           +----------------------------+
                           |                            |
                           v                            v
                  [Projector Trigger]           [Saga Trigger]
                  (Pub/Sub push / SQS)          (Pub/Sub push / SQS)
```

## Feature Flags (Cargo.toml)

```toml
[features]
# Cloud-native messaging
gcp_pubsub = ["dep:google-cloud-pubsub"]
aws_sqs = ["dep:aws-sdk-sqs", "dep:aws-sdk-sns"]
aws_kinesis = ["dep:aws-sdk-kinesis"]

# Cloud-native storage
gcp_bigtable = ["dep:google-cloud-bigtable"]
aws_dynamodb = ["dep:aws-sdk-dynamodb"]

# Existing Redis storage works with cloud-managed Redis
# redis = ["dep:redis"]  # Already defined - use with Memorystore/ElastiCache

# Full serverless bundles
cloudrun = ["gcp_pubsub", "gcp_bigtable"]
lambda = ["aws_sqs", "aws_dynamodb", "dep:aws-lambda-runtime"]
```

## Implementation Phases

### Phase 1: Cloud-Native Messaging

**Files to create:**
- `src/bus/gcp_pubsub/mod.rs` - Google Pub/Sub EventBus
- `src/bus/aws_sqs/mod.rs` - AWS SNS publisher + SQS types
- `src/bus/aws_kinesis/mod.rs` - AWS Kinesis EventBus (Kafka-like semantics)

**Key patterns:**
- Implement `EventBus` trait (publish only - push handles subscribe)
- Topic per domain: `angzarr-events-{domain}`
- Message attributes: `domain`, `root_id`, `correlation_id`

**Update:** `src/bus/mod.rs`
- Add `MessagingType::GcpPubSub`, `MessagingType::AwsSqs`, `MessagingType::AwsKinesis`
- Add config structs and factory branches

**AWS messaging choice:**
- **SQS/SNS**: Simpler, queue-based, good for decoupled consumers
- **Kinesis**: Kafka-like ordered streams, better for event replay and multiple consumers reading same data

### Phase 2: Service Discovery Abstraction

**Files to modify:**
- `src/discovery/mod.rs` - Extract `ServiceDiscovery` trait

**Files to create:**
- `src/discovery/env_urls.rs` - Environment variable based discovery
- `src/discovery/gcp_mesh.rs` - Cloud Service Mesh discovery (optional)

#### Current K8s Discovery

Angzarr uses K8s label-based discovery (`src/discovery/k8s/mod.rs`):
- Watches Service resources via K8s API
- Filters by labels: `app.kubernetes.io/component`, `angzarr.io/domain`
- Builds gRPC client URLs from service DNS names
- Caches connections per domain

#### Option 1: Environment URL Discovery (Recommended for Serverless)

Simplest approach - service URLs passed via environment variables:

```rust
// src/discovery/env_urls.rs
pub struct EnvUrlDiscovery {
    aggregates: HashMap<String, String>,   // domain -> URL
    projectors: Vec<String>,
    sagas: HashMap<String, Vec<String>>,   // source_domain -> URLs
}

impl EnvUrlDiscovery {
    pub fn from_env() -> Result<Self, Error> {
        // Parse environment variables:
        // ANGZARR_AGGREGATE_CART=https://cart-aggregate.run.app
        // ANGZARR_AGGREGATE_ORDER=https://order-aggregate.run.app
        // ANGZARR_PROJECTOR_1=https://web-projector.run.app
        // ANGZARR_SAGA_ORDER_1=https://fulfillment-saga.run.app
    }
}
```

**Pros**: Simple, works everywhere, no cloud-specific dependencies
**Cons**: Manual configuration, no dynamic updates

#### Option 2: Cloud Run Service Mesh (GCP)

Cloud Service Mesh provides automatic discovery:

```rust
// src/discovery/gcp_mesh.rs
pub struct CloudMeshDiscovery {
    mesh_name: String,
}

impl ServiceDiscovery for CloudMeshDiscovery {
    async fn get_aggregate(&self, domain: &str) -> Result<Client> {
        // Services registered in mesh get predictable URLs:
        // https://{service-name}.{mesh-name}.mesh.internal
        let url = format!("https://{}-aggregate.{}.mesh.internal", domain, self.mesh_name);
        connect(url).await
    }
}
```

**OpenTofu:**
```hcl
resource "google_network_services_mesh" "angzarr" {
  name = "angzarr"
}

resource "google_cloud_run_v2_service" "cart_aggregate" {
  name = "cart-aggregate"

  template {
    service_mesh {
      mesh = google_network_services_mesh.angzarr.id
    }
  }
}
```

**Pros**: Automatic discovery, traffic management, mTLS
**Cons**: Pre-GA, cold start latency impact, GCP-only

#### Option 3: AWS Service Discovery (AWS Cloud Map)

AWS Cloud Map provides DNS-based discovery:

```rust
// src/discovery/aws_cloudmap.rs
pub struct CloudMapDiscovery {
    namespace: String,
    client: ServiceDiscoveryClient,
}

impl ServiceDiscovery for CloudMapDiscovery {
    async fn get_aggregate(&self, domain: &str) -> Result<Client> {
        // Discover instances registered in Cloud Map
        let instances = self.client
            .discover_instances()
            .namespace_name(&self.namespace)
            .service_name(&format!("{}-aggregate", domain))
            .send()
            .await?;

        // Get URL from instance attributes
        let url = instances.instances[0].attributes["url"].clone();
        connect(url).await
    }
}
```

**OpenTofu:**
```hcl
resource "aws_service_discovery_private_dns_namespace" "angzarr" {
  name = "angzarr.local"
  vpc  = aws_vpc.main.id
}

resource "aws_service_discovery_service" "cart_aggregate" {
  name = "cart-aggregate"

  dns_config {
    namespace_id = aws_service_discovery_private_dns_namespace.angzarr.id

    dns_records {
      ttl  = 10
      type = "A"
    }
  }
}
```

**Pros**: Native AWS integration, DNS-based, works with Lambda
**Cons**: Requires VPC, AWS-only

#### Option 4: Consul (Multi-Cloud)

HashiCorp Consul works across clouds:

```rust
// src/discovery/consul.rs
pub struct ConsulDiscovery {
    client: ConsulClient,
}

impl ServiceDiscovery for ConsulDiscovery {
    async fn get_aggregate(&self, domain: &str) -> Result<Client> {
        let service = format!("{}-aggregate", domain);
        let instances = self.client.health().service(&service, true).await?;
        let url = format!("https://{}:{}", instances[0].address, instances[0].port);
        connect(url).await
    }
}
```

**Pros**: Multi-cloud, mature, service mesh features
**Cons**: Additional infrastructure to manage

#### Discovery Configuration

```yaml
# config.yaml
discovery:
  type: env_urls  # or: k8s, gcp_mesh, aws_cloudmap, consul

  env_urls:
    aggregates:
      cart: ${CART_AGGREGATE_URL}
      order: ${ORDER_AGGREGATE_URL}

  gcp_mesh:
    mesh_name: angzarr

  aws_cloudmap:
    namespace: angzarr.local
    region: us-east-1

  consul:
    address: http://consul:8500
```

#### Recommendation by Platform

| Platform | Primary | Alternative |
|----------|---------|-------------|
| K8s | K8s labels (existing) | Consul |
| Cloud Run | Env URLs | Cloud Service Mesh |
| Lambda | Env URLs | AWS Cloud Map |
| Multi-cloud | Consul | Env URLs |

### Phase 3: Cloud-Native Storage

**Files to create:**
- `src/storage/gcp_bigtable.rs` - Bigtable EventStore/SnapshotStore
- `src/storage/aws_dynamodb.rs` - DynamoDB EventStore/SnapshotStore

**Bigtable structure:**
```
Table: angzarr-events
Row key: {domain}#{root_id}#{sequence:08d}
Column family: event
  - data: protobuf bytes
  - created_at: timestamp

Table: angzarr-snapshots
Row key: {domain}#{root_id}
Column family: snapshot
  - data: protobuf bytes
  - sequence: u32
```

**DynamoDB structure:**
```
Table: angzarr-events
PK: {domain}#{root_id}
SK: {sequence}
```

### Cloud-Managed Redis (Alternative Storage)

The existing `redis` feature (`src/storage/redis.rs`) works with cloud-managed Redis services.
This provides a simpler alternative to Bigtable/DynamoDB for smaller workloads.

#### GCP Memorystore for Redis/Valkey

[Memorystore](https://cloud.google.com/memorystore) offers managed Redis with persistence:

- **Valkey** (recommended): 99.99% SLA, cross-region replication, managed backups
- **Redis Cluster**: Scale to 250 nodes, terabytes of keyspace
- **Persistence**: RDB snapshots (1h/6h/12h/24h) or AOF for durability

**OpenTofu:**
```hcl
resource "google_redis_instance" "angzarr" {
  name               = "angzarr-events"
  memory_size_gb     = 5
  region             = "us-central1"
  tier               = "STANDARD_HA"  # High availability

  persistence_config {
    persistence_mode    = "RDB"
    rdb_snapshot_period = "TWELVE_HOURS"
  }
}
```

**Configuration:**
```yaml
storage:
  type: redis
  redis:
    url: "redis://${MEMORYSTORE_HOST}:6379"
    key_prefix: "angzarr"
```

#### AWS ElastiCache Serverless

[ElastiCache Serverless](https://aws.amazon.com/elasticache/serverless/) for Redis/Valkey:

- **Auto-scaling**: Pay only for what you use
- **Multi-AZ**: Automatic replication across availability zones
- **Encryption**: TLS in transit, encrypted at rest by default
- **Valkey**: 33% reduced price, 100MB minimum (vs 1GB for Redis)

**OpenTofu:**
```hcl
resource "aws_elasticache_serverless_cache" "angzarr" {
  engine = "valkey"
  name   = "angzarr-events"

  cache_usage_limits {
    data_storage {
      minimum = 1
      maximum = 100
      unit    = "GB"
    }
  }

  security_group_ids = [aws_security_group.redis.id]
  subnet_ids         = var.subnet_ids
}
```

**Note**: ElastiCache Serverless doesn't support Global Datastore (cross-region replication).
For multi-region, use self-managed ElastiCache clusters.

#### When to Use Redis vs Bigtable/DynamoDB

| Criteria | Redis | Bigtable/DynamoDB |
|----------|-------|-------------------|
| Scale | < 1TB, < 1M events/day | Unlimited |
| Latency | Sub-millisecond | Single-digit millisecond |
| Multi-region | Limited (Memorystore Valkey) | Native support |
| Cost | Lower for small workloads | Better at scale |
| Complexity | Simpler | More configuration |

### Phase 4: Serverless Binaries

**Files to create:**
- `src/bin/angzarr_gateway_lambda.rs` - Lambda wrapper for gateway
- `src/bin/angzarr_projector_trigger.rs` - Push-triggered projector handler
- `src/bin/angzarr_saga_trigger.rs` - Push-triggered saga handler

**Key change:** Convert long-running consumers to HTTP-triggered handlers:

```rust
// Cloud Run: HTTP endpoint for Pub/Sub push
async fn handle_pubsub_push(Json(msg): Json<PubSubPushMessage>) -> StatusCode {
    let event_book = EventBook::decode(&msg.message.data)?;
    projector_handler.handle(Arc::new(event_book)).await?;
    StatusCode::OK
}

// Lambda: SQS trigger
async fn handler(event: LambdaEvent<SqsEvent>) -> Result<(), Error> {
    for record in event.payload.records {
        let event_book = EventBook::decode(&record.body)?;
        saga_handler.handle(Arc::new(event_book)).await?;
    }
    Ok(())
}
```

**Stream service:** Eliminate for serverless (clients poll instead of stream)

### Phase 5: OpenTofu Infrastructure

**Directory structure:**
```
deploy/tofu/
├── modules/
│   ├── cloudrun/
│   │   ├── main.tf        # Cloud Run services
│   │   ├── pubsub.tf      # Topics and push subscriptions
│   │   ├── bigtable.tf    # Database
│   │   └── variables.tf
│   └── lambda/
│       ├── main.tf        # Lambda functions
│       ├── sns_sqs.tf     # Topics, queues, subscriptions
│       ├── dynamodb.tf    # Tables
│       ├── api_gateway.tf # HTTP routing
│       └── variables.tf
└── environments/
    ├── gcp-serverless/
    └── aws-serverless/
```

### Phase 6: Configuration

**New config options:**
```yaml
# config.cloudrun.yaml
storage:
  type: gcp_bigtable
  gcp_bigtable:
    project_id: my-project
    instance_id: angzarr

messaging:
  type: gcp_pubsub
  gcp_pubsub:
    project_id: my-project
    topic_prefix: angzarr-events

discovery:
  type: env_urls
```

```yaml
# config.lambda.yaml
storage:
  type: aws_dynamodb
  aws_dynamodb:
    table_name: angzarr-events
    region: us-east-1

messaging:
  type: aws_sqs
  aws_sqs:
    region: us-east-1
    topic_arn_prefix: arn:aws:sns:us-east-1:123456789:angzarr-events-

discovery:
  type: env_urls
```

## Critical Files

| File | Change |
|------|--------|
| `Cargo.toml` | Add feature flags and dependencies |
| `src/bus/mod.rs` | Add PubSub/SQS messaging types |
| `src/bus/gcp_pubsub/mod.rs` | New: Pub/Sub implementation |
| `src/bus/aws_sqs/mod.rs` | New: SQS/SNS implementation |
| `src/bus/aws_kinesis/mod.rs` | New: Kinesis implementation |
| `src/discovery/mod.rs` | Abstract ServiceDiscovery trait |
| `src/discovery/env_urls.rs` | New: Environment URL discovery |
| `src/storage/mod.rs` | Add gcp_bigtable/aws_dynamodb types |
| `src/storage/gcp_bigtable.rs` | New: Bigtable implementation |
| `src/storage/aws_dynamodb.rs` | New: DynamoDB implementation |
| `src/bin/angzarr_projector_trigger.rs` | New: Push-triggered projector |
| `src/bin/angzarr_saga_trigger.rs` | New: Push-triggered saga |
| `src/bin/angzarr_gateway_lambda.rs` | New: Lambda gateway wrapper |

## Cold Start Optimization

```toml
# Cargo.toml [profile.release]
strip = true
lto = true
codegen-units = 1
panic = "abort"
```

Connection pooling via `OnceCell` for reuse across invocations.

## Dependencies

```toml
# GCP
google-cloud-pubsub = { version = "0.25", optional = true }
google-cloud-bigtable = { version = "0.25", optional = true }

# AWS
aws-lambda-runtime = { version = "0.13", optional = true }
aws-sdk-sqs = { version = "1.50", optional = true }
aws-sdk-sns = { version = "1.50", optional = true }
aws-sdk-kinesis = { version = "1.50", optional = true }
aws-sdk-dynamodb = { version = "1.50", optional = true }

# HTTP server for Cloud Run triggers
axum = { version = "0.7", optional = true }
```

## Advanced Topics (Future Work)

### Cloud Run Service Mesh Discovery

[Cloud Service Mesh](https://cloud.google.com/service-mesh/docs/overview) provides native service discovery for Cloud Run:

- **Unified mesh**: Works across Cloud Run, GKE, and VMs
- **Custom URLs**: Call services via `service-name.mesh.internal` instead of `*.run.app`
- **Automatic auth**: Cloud Run automatically authenticates service-to-service calls
- **Traffic management**: Weighted routing, global load balancing

**Implementation approach:**
```rust
// src/discovery/gcp_mesh.rs
pub struct CloudServiceMeshDiscovery {
    project_id: String,
    mesh_name: String,
}

impl ServiceDiscovery for CloudServiceMeshDiscovery {
    async fn get_aggregate(&self, domain: &str) -> Result<...> {
        // Use mesh URL: https://{domain}-aggregate.mesh.internal
    }
}
```

**OpenTofu:**
```hcl
resource "google_network_services_mesh" "angzarr" {
  name = "angzarr-mesh"
}

resource "google_cloud_run_v2_service" "aggregate" {
  # Enable mesh
  annotations = {
    "run.googleapis.com/mesh" = google_network_services_mesh.angzarr.id
  }
}
```

**Limitation**: Pre-GA feature, may have cold start latency impact.

### Multi-Region Replication

#### Bigtable Multi-Region

[Bigtable replication](https://cloud.google.com/bigtable/docs/replication-overview) supports up to 8 regions with 99.999% availability:

- **Multi-primary**: Write to any region
- **Row affinity routing**: Consistent reads for same row key
- **Automatic failover**: Traffic reroutes on regional outage

**Implementation:**
```rust
// src/storage/gcp_bigtable.rs
pub struct BigtableConfig {
    pub project_id: String,
    pub instance_id: String,
    pub app_profile: String,  // For routing policy
}

// App profile options:
// - single-cluster: Always route to specific cluster
// - multi-cluster: Route to nearest cluster
// - row-affinity: Sticky routing by row key (recommended for event sourcing)
```

**OpenTofu:**
```hcl
resource "google_bigtable_instance" "angzarr" {
  name = "angzarr"

  cluster {
    cluster_id   = "us-central1"
    zone         = "us-central1-a"
    num_nodes    = 1
  }

  cluster {
    cluster_id   = "europe-west1"
    zone         = "europe-west1-b"
    num_nodes    = 1
  }
}

resource "google_bigtable_app_profile" "row_affinity" {
  instance        = google_bigtable_instance.angzarr.name
  app_profile_id  = "row-affinity"

  multi_cluster_routing_use_any {
    cluster_ids = ["us-central1", "europe-west1"]
  }

  data_boost_isolation_read_only {
    compute_billing_owner = "HOST_PAYS"
  }
}
```

#### DynamoDB Global Tables

AWS DynamoDB supports [global tables](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GlobalTables.html) for multi-region:

- **Active-active**: Write to any region
- **Conflict resolution**: Last-writer-wins by timestamp
- **Automatic replication**: Sub-second replication latency

```hcl
resource "aws_dynamodb_table" "events" {
  name         = "angzarr-events"
  billing_mode = "PAY_PER_REQUEST"

  replica {
    region_name = "us-east-1"
  }

  replica {
    region_name = "eu-west-1"
  }
}
```

### Managed WebSocket for Streaming

Current plan eliminates streaming for serverless. To restore it:

#### AWS API Gateway WebSocket APIs

[API Gateway WebSockets](https://aws.amazon.com/blogs/compute/building-serverless-multi-region-websocket-apis/) provide managed bidirectional communication:

```
[Client] <--WebSocket--> [API Gateway] <--Lambda--> [DynamoDB connections]
                                              |
                                              v
                              [API Gateway Management API]
```

**Implementation:**
- `$connect`: Lambda stores connection ID in DynamoDB
- `$disconnect`: Lambda removes connection ID
- `$default`: Lambda processes messages
- Send to client: Call `POST /@connections/{connectionId}`

**Files to create:**
- `src/bin/angzarr_websocket_connect.rs` - Connection handler
- `src/bin/angzarr_websocket_message.rs` - Message handler
- `src/websocket/connection_store.rs` - DynamoDB connection tracking

**OpenTofu:**
```hcl
resource "aws_apigatewayv2_api" "websocket" {
  name                       = "angzarr-events"
  protocol_type              = "WEBSOCKET"
  route_selection_expression = "$request.body.action"
}

resource "aws_apigatewayv2_route" "connect" {
  api_id    = aws_apigatewayv2_api.websocket.id
  route_key = "$connect"
  target    = "integrations/${aws_apigatewayv2_integration.connect.id}"
}
```

#### GCP Alternatives

Cloud Run doesn't natively support WebSocket push. Options:
1. **Firebase Realtime Database**: Push updates via Firebase SDK
2. **Firestore listeners**: Real-time sync via Firestore
3. **Cloud Pub/Sub + Server-Sent Events**: Long-polling alternative

#### Third-Party Options

- **Ably**: Managed pub/sub with WebSocket support
- **Pusher**: Real-time messaging service
- **Momento Topics**: Serverless pub/sub (see detailed evaluation below)

### Momento Topics Evaluation

[Momento Topics](https://www.gomomento.com/services/momento-topics) is a serverless pub/sub service designed for real-time communication.

#### Pricing
- **$0.15/GB** data transferred
- **50GB free tier** monthly
- No infrastructure costs, no topic provisioning

#### Technical Characteristics

| Aspect | Details |
|--------|---------|
| **Model** | Fire-and-forget (no persistence) |
| **Latency** | Single-digit milliseconds at scale |
| **Scale** | Millions of messages/second |
| **Connection** | Stateful gRPC for subscribers |
| **Delivery** | No guarantees (best-effort) |
| **Configuration** | Zero - no topics to create |

#### Serverless Compatibility

**Critical limitation**: Subscribers require long-lived gRPC connections.

| Operation | Lambda/Cloud Functions | Cloud Run/Fargate |
|-----------|------------------------|-------------------|
| Publish | ✅ Works (stateless) | ✅ Works |
| Subscribe | ❌ Not possible | ✅ Works |

This makes Momento Topics unsuitable for angzarr's serverless projectors/sagas which need to receive events.

#### Comparison with AWS SNS/SQS

| Feature | Momento Topics | SNS | SQS |
|---------|---------------|-----|-----|
| Pattern | Live connections | Push webhooks | Pull queue |
| Persistence | None | None | Up to 14 days |
| Lambda subscribe | ❌ | ✅ | ✅ |
| Latency | <10ms | ~100ms | Higher (polling) |
| Configuration | Zero | Moderate | Moderate |

#### Use Cases

**Good fit:**
- Real-time chat/multiplayer games
- Live dashboards with browser clients
- WebSocket replacement for client apps
- Low-latency notifications where loss is tolerable

**Not suitable for:**
- Event sourcing (no persistence, no delivery guarantees)
- Serverless event handlers (Lambda/Cloud Functions can't subscribe)
- Saga coordination (requires guaranteed delivery)
- Audit trails (messages are discarded)

#### Recommendation for Angzarr

**Do not use** Momento Topics for core event distribution because:
1. Fire-and-forget model incompatible with event sourcing requirements
2. No delivery guarantees - events could be lost
3. Lambda/serverless functions cannot subscribe
4. Projectors and sagas require persistent, reliable message delivery

**Potential use**: Client-facing real-time notifications layer.
If angzarr needs to push live updates to browser/mobile clients:

```
[Event Store] -> [Pub/Sub/SNS] -> [Notification Service] -> [Momento Topics] -> [Clients]
                                         |
                                   (filters, transforms)
```

This isolates Momento Topics from the critical event sourcing path while leveraging its low-latency client connections.

#### Customer Adoption

Per Momento's marketing:
- Paramount, NTT Docomo, ProSieben, Wyze use Momento platform
- Limited independent reviews available (AWS Marketplace shows no reviews yet)
- Positioned for mission-critical workloads at "billions of operations"

#### Sources
- [Momento Topics Overview](https://www.gomomento.com/services/momento-topics)
- [InfoQ: Serverless Event Messaging](https://www.infoq.com/news/2023/04/serverless-pubsub-momento-topics/)
- [How Momento Built Topics](https://www.gomomento.com/blog/how-we-built-momento-topics-a-serverless-messaging-service/)

### Lambda@Edge / Cloud CDN

For global edge distribution:

**AWS CloudFront + Lambda@Edge:**
- Execute at edge locations
- Lower latency for reads
- Use for: Query API, event replay

**GCP Cloud CDN:**
- Cache at edge
- Use with Cloud Run for static content
- Limited compute at edge (use Cloud Functions gen2)

### Sources

- [Cloud Service Mesh Overview](https://cloud.google.com/service-mesh/docs/overview)
- [Configure Cloud Service Mesh for Cloud Run](https://cloud.google.com/service-mesh/docs/configure-cloud-service-mesh-for-cloud-run)
- [Bigtable Replication Overview](https://cloud.google.com/bigtable/docs/replication-overview)
- [Building Serverless Multi-Region WebSocket APIs](https://aws.amazon.com/blogs/compute/building-serverless-multi-region-websocket-apis/)
- [API Gateway WebSocket Tutorial](https://www.serverless.com/framework/docs/providers/aws/events/websocket)
