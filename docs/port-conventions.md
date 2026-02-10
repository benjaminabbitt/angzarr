# Port Conventions

Angzarr uses consistent port numbering across all deployment modes.

## Infrastructure Ports

Core framework services use fixed ports:

| Service | Port | NodePort | Description |
|---------|------|----------|-------------|
| Aggregate Coordinator | 1310 | 31310 | Per-domain command handling |
| Stream gRPC | 1340 | 31340 | Event streaming |
| Topology REST API | 9099 | - | Topology visualization |

Clients connect directly to per-domain aggregate coordinators via K8s DNS (e.g., `angzarr-order.angzarr.svc.cluster.local:1310`).

---

## Business Logic Port Scheme

Business logic services use a ten-port-per-pod scheme for consistent addressing.

Each pod uses a base port (e.g., 50050, 50060, 50070) with offsets 0-9:

| Offset | Purpose | Typically Exposed | Description |
|--------|---------|-------------------|-------------|
| 0 | Coordinator gRPC | Yes | Angzarr sidecar coordinator (aggregate, projector, saga) |
| 1 | REST Proxy | Optional | REST → gRPC proxy for HTTP clients |
| 2 | Coordinator Debug | No | Angzarr sidecar debug/diagnostics endpoint |
| 3 | Client Logic | No | client logic gRPC (internal sidecar-to-logic communication) |
| 4 | Client Debug | No | client logic debug/diagnostics endpoint |
| 5-8 | Reserved | No | Future use |
| 9 | Control/Meta UI | Optional | Admin UI, metrics dashboard, or control plane |

## Port Ranges by Language

Each language example uses a distinct range to allow concurrent local development:

| Language | Base Range | Aggregates | Sagas | Projectors |
|----------|------------|------------|-------|------------|
| Rust | 50050-50199 | 50050-50109 | 50110-50139 | 50140-50159 |
| Go | 50200-50349 | 50200-50259 | 50260-50289 | 50290-50309 |
| Python | 50400-50549 | 50400-50459 | 50460-50489 | 50490-50509 |

## Rust Port Assignments

### Aggregates (50050-50109)

| Service | Base | Coordinator | REST | Debug | Logic | Logic Debug | Control |
|---------|------|-------------|------|-------|-------|-------------|---------|
| Customer | 50050 | 50050 | 50051 | 50052 | 50053 | 50054 | 50059 |
| Product | 50060 | 50060 | 50061 | 50062 | 50063 | 50064 | 50069 |
| Inventory | 50070 | 50070 | 50071 | 50072 | 50073 | 50074 | 50079 |
| Order | 50080 | 50080 | 50081 | 50082 | 50083 | 50084 | 50089 |
| Cart | 50090 | 50090 | 50091 | 50092 | 50093 | 50094 | 50099 |
| Fulfillment | 50100 | 50100 | 50101 | 50102 | 50103 | 50104 | 50109 |

### Sagas (50110-50139)

| Service | Base | Coordinator | REST | Debug | Logic | Logic Debug | Control |
|---------|------|-------------|------|-------|-------|-------------|---------|
| Loyalty Earn | 50110 | 50110 | 50111 | 50112 | 50113 | 50114 | 50119 |
| Fulfillment | 50120 | 50120 | 50121 | 50122 | 50123 | 50124 | 50129 |
| Cancellation | 50130 | 50130 | 50131 | 50132 | 50133 | 50134 | 50139 |

### Projectors (50140-50159)

| Service | Base | Coordinator | REST | Debug | Logic | Logic Debug | Control |
|---------|------|-------------|------|-------|-------|-------------|---------|
| Accounting | 50140 | 50140 | 50141 | 50142 | 50143 | 50144 | 50149 |
| Web | 50150 | 50150 | 50151 | 50152 | 50153 | 50154 | 50159 |

## Design Rationale

### Why ten ports?

1. **Coordinator (offset 0)**: The primary gRPC endpoint that external clients and the message bus connect to. Always exposed.

2. **REST Proxy (offset 1)**: Optional HTTP/REST proxy for clients that cannot use gRPC directly. Proxies to the coordinator.

3. **Coordinator Debug (offset 2)**: Health checks, metrics, and debugging endpoints for the sidecar. Exposed only in development or for monitoring.

4. **Client Logic (offset 3)**: Internal communication between sidecar and client logic. **Should not be exposed externally** - the sidecar handles all external communication.

5. **Client Debug (offset 4)**: client logic health/debug endpoints. Useful for development troubleshooting.

6. **Reserved (offsets 5-8)**: Available for future expansion without restructuring.

7. **Control UI (offset 9)**: Optional admin interface, projection viewers, or control plane UI.

### Why separate ranges per language?

- Enables running Rust, Go, and Python implementations simultaneously for comparison testing
- Prevents port conflicts during local development
- Clear ownership when debugging multi-language deployments

## Kubernetes Considerations

In Kubernetes, container ports are typically remapped:

```yaml
# values.yaml example
applications:
  business:
    - name: customer
      ports:
        coordinator: 50050   # Exposed via Service
        rest: 50051          # Exposed if REST clients needed
        debug: 50052         # Exposed only if needed
        # Logic port (50053) stays internal to pod
```

The mesh/ingress routes to the coordinator port (offset 0). Internal sidecar-to-logic communication uses localhost within the pod.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | varies | Coordinator gRPC port (offset 0) |
| `REST_PORT` | PORT+1 | REST proxy port (offset 1) |
| `DEBUG_PORT` | PORT+2 | Debug endpoint port (offset 2) |
| `TARGET_PORT` | PORT+3 | client logic port (offset 3) |

## See Also

- [Command Handlers](components/aggregate/aggregate.md) - Aggregate sidecar architecture
- [Projectors](components/projector/projectors.md) - Projector sidecar architecture
- [Sagas](components/saga/sagas.md) - Saga sidecar architecture
