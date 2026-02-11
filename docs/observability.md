# Observability

Angzarr provides full observability through OpenTelemetry, exporting traces, metrics, and logs via OTLP.

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Angzarr        │     │  OTel Collector │     │  Backends       │
│  Sidecars       │────▶│  (OTLP)         │────▶│                 │
│                 │     │                 │     │  Tempo (traces) │
│  - Coordinator  │     │  Processors:    │     │  Prometheus     │
│  - Projector    │     │  - batch        │     │  Loki (logs)    │
│  - Saga         │     │  - memory_limit │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                                ┌─────────────────┐
                                                │  Grafana        │
                                                │  - Dashboards   │
                                                │  - Trace viewer │
                                                │  - Log explorer │
                                                └─────────────────┘
```

## Feature Flag

Enable OpenTelemetry with the `otel` feature:

```bash
cargo build --features otel
```

Without this flag, only console logging via `tracing` is available.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_SERVICE_NAME` | Service name in traces/metrics | `angzarr` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector endpoint | `http://localhost:4317` |
| `OTEL_RESOURCE_ATTRIBUTES` | Additional resource attributes | - |
| `RUST_LOG` | Log level filter | `info` |

Example:
```bash
export OTEL_SERVICE_NAME=angzarr-order
export OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
export OTEL_RESOURCE_ATTRIBUTES=deployment.environment=prod,service.version=1.0.0
```

## Metrics

All metrics use OTel semantic conventions with dot-separated names. The OTel Collector converts to Prometheus format (underscores).

### Command Pipeline

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `angzarr.command.duration` | Histogram | domain, outcome | Command handling latency |
| `angzarr.command.total` | Counter | domain, outcome | Total commands processed |

### Event Bus

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `angzarr.bus.publish.duration` | Histogram | bus_type, domain | Publish operation latency |
| `angzarr.bus.publish.total` | Counter | bus_type, domain | Total publish operations |

### Orchestration

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `angzarr.saga.duration` | Histogram | name | Saga execution time |
| `angzarr.saga.retry.total` | Counter | name | Saga retry attempts |
| `angzarr.saga.compensation.total` | Counter | name | Compensations triggered |
| `angzarr.pm.duration` | Histogram | name | Process manager execution time |
| `angzarr.projector.duration` | Histogram | name | Projector handling time |

### Labels

| Label | Values |
|-------|--------|
| `domain` | Aggregate domain (e.g., `order`, `inventory`) |
| `outcome` | `success`, `rejected`, `error` |
| `bus_type` | `amqp`, `kafka`, `channel`, `ipc` |
| `component` | `aggregate`, `saga`, `projector`, `process_manager` |
| `name` | Component instance name |

## Traces

Angzarr propagates W3C TraceContext headers across gRPC boundaries, enabling distributed tracing through:

- Client → Gateway → Coordinator → Business Logic
- Event Bus → Projector/Saga Coordinator → Business Logic
- Saga → Target Aggregate

Each span includes:
- `domain` attribute
- `correlation_id` (if present)
- Event/command type URLs

## Kubernetes Deployment

### Deploy Observability Stack

```bash
# Add Helm repos
helm repo add grafana https://grafana.github.io/helm-charts
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo add open-telemetry https://open-telemetry.github.io/opentelemetry-helm-charts
helm repo update

# Deploy observability stack
helm install angzarr-otel ./deploy/helm/observability -n monitoring --create-namespace
```

This deploys:
- **OTel Collector** - OTLP receiver on port 4317 (gRPC) and 4318 (HTTP)
- **Tempo** - Distributed tracing backend
- **Prometheus** - Metrics via remote write
- **Loki** - Log aggregation
- **Grafana** - Visualization (NodePort 30300, admin/angzarr)

### Enable OTel on Angzarr

```bash
# Deploy with observability overlay
helm install angzarr ./deploy/helm/angzarr \
  -f values-local.yaml \
  -f values-observability.yaml \
  -n angzarr
```

The overlay sets `OTEL_EXPORTER_OTLP_ENDPOINT` on all sidecar containers.

## Grafana Dashboards

Pre-built dashboards are deployed automatically:

### Command Pipeline
- Command throughput by domain
- Latency percentiles (p50/p95/p99)
- Error rate and rejection reasons
- Success/failure breakdown

### Event Bus
- Publish throughput by bus type
- Publish latency distribution
- Failed publishes

### Orchestration
- Saga execution times
- Retry rates
- Compensation events
- Process manager durations

### Topology
- Live system topology graph (requires Infinity plugin)
- Component relationships
- Event flow visualization

## Local Development

### Quick Start with Docker Compose

```yaml
# docker-compose.otel.yaml
services:
  otel-collector:
    image: otel/opentelemetry-collector-contrib:latest
    ports:
      - "4317:4317"   # OTLP gRPC
      - "4318:4318"   # OTLP HTTP
    volumes:
      - ./otel-config.yaml:/etc/otel/config.yaml
    command: ["--config=/etc/otel/config.yaml"]

  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # UI
      - "4317"         # OTLP (internal)
    environment:
      - COLLECTOR_OTLP_ENABLED=true
```

```yaml
# otel-config.yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317

exporters:
  otlp:
    endpoint: jaeger:4317
    tls:
      insecure: true
  debug:
    verbosity: detailed

service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [otlp, debug]
```

### Standalone Mode

For standalone development without a collector:

```bash
# Logs only (no OTel)
RUST_LOG=debug cargo run --features standalone --bin angzarr_standalone

# With OTel to local collector
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317 \
  cargo run --features "standalone otel" --bin angzarr_standalone
```

## Alerting Examples

### Prometheus Alert Rules

```yaml
groups:
  - name: angzarr
    rules:
      - alert: HighCommandLatency
        expr: histogram_quantile(0.99, rate(angzarr_command_duration_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High command latency on {{ $labels.domain }}"

      - alert: SagaCompensationSpike
        expr: rate(angzarr_saga_compensation_total[5m]) > 0.1
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Saga compensations increasing - check domain boundaries"

      - alert: CommandRejectionRate
        expr: |
          sum(rate(angzarr_command_total{outcome="rejected"}[5m])) by (domain)
          /
          sum(rate(angzarr_command_total[5m])) by (domain) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High rejection rate on {{ $labels.domain }}"
```

## Troubleshooting

### No Metrics in Prometheus

1. Verify OTel feature is enabled: `cargo build --features otel`
2. Check `OTEL_EXPORTER_OTLP_ENDPOINT` is set correctly
3. Verify collector is receiving data: check collector logs or debug exporter
4. Confirm Prometheus remote write is configured in collector

### No Traces in Tempo/Jaeger

1. Verify trace propagation: check `traceparent` header in gRPC metadata
2. Check collector pipeline includes `traces`
3. Verify Tempo/Jaeger OTLP endpoint in collector config

### Missing Correlation

Ensure `correlation_id` is set on initial commands. The framework propagates it through sagas and process managers automatically.

## References

- [OpenTelemetry Rust SDK](https://github.com/open-telemetry/opentelemetry-rust)
- [OTel Collector Configuration](https://opentelemetry.io/docs/collector/configuration/)
- [Grafana Tempo](https://grafana.com/docs/tempo/latest/)
- [Prometheus Remote Write](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#remote_write)
