---
sidebar_position: 2
---

# Observability

Angzarr provides full observability through OpenTelemetry, exporting traces, metrics, and logs via OTLP.

---

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

---

## Feature Flag

Enable OpenTelemetry with the `otel` feature:

```bash
cargo build --features otel
```

Without this flag, only console logging via `tracing` is available.

---

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_SERVICE_NAME` | Service name in traces/metrics | `angzarr` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | Collector endpoint | `http://localhost:4317` |
| `OTEL_RESOURCE_ATTRIBUTES` | Additional resource attributes | - |
| `RUST_LOG` | Log level filter | `info` |

```bash
export OTEL_SERVICE_NAME=angzarr-order
export OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317
export OTEL_RESOURCE_ATTRIBUTES=deployment.environment=prod,service.version=1.0.0
```

---

## Metrics

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
| `domain` | Aggregate domain (e.g., `player`, `table`) |
| `outcome` | `success`, `rejected`, `error` |
| `bus_type` | `amqp`, `kafka`, `channel`, `ipc` |
| `component` | `aggregate`, `saga`, `projector`, `process_manager` |
| `name` | Component instance name |

---

## Traces

Angzarr propagates W3C TraceContext headers across gRPC boundaries, enabling distributed tracing through:

- Client → Coordinator → Business Logic
- Event Bus → Projector/Saga → Business Logic
- Saga → Target Aggregate

Each span includes:
- `domain` attribute
- `correlation_id` (if present)
- Event/command type URLs

---

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
- **OTel Collector** — OTLP receiver on port 4317 (gRPC) and 4318 (HTTP)
- **Tempo** — Distributed tracing backend
- **Prometheus** — Metrics via remote write
- **Loki** — Log aggregation
- **Grafana** — Visualization (NodePort 30300)

### Enable OTel on Angzarr

```bash
helm install angzarr ./deploy/helm/angzarr \
  -f values-local.yaml \
  -f values-observability.yaml \
  -n angzarr
```

---

## Grafana Dashboards

Pre-built dashboards are deployed automatically:

- **Command Pipeline** — Throughput, latency percentiles, error rates
- **Event Bus** — Publish throughput, latency distribution
- **Orchestration** — Saga execution, retry rates, compensations
- **Topology** — Live system topology graph

---

## Alerting Examples

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
          summary: "Saga compensations increasing"
```

---

## Next Steps

- **[Infrastructure](/operations/infrastructure)** — Helm chart deployment
- **[Testing](/operations/testing)** — Integration tests with observability
