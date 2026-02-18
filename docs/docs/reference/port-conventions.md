---
sidebar_position: 2
---

# Port Conventions

Angzarr uses consistent port numbering across all deployment modes.

---

## Infrastructure Ports

Core framework services use fixed ports:

| Service | Port | NodePort | Description |
|---------|------|----------|-------------|
| Aggregate Coordinator | 1310 | 31310 | Per-domain command handling |
| Stream gRPC | 1340 | 31340 | Event streaming |
| Topology REST API | 9099 | - | Topology visualization |

---

## Business Logic Port Scheme

Business logic services use a **ten-port-per-pod** scheme for consistent addressing.

Each pod uses a base port (e.g., 50050, 50060) with offsets 0-9:

| Offset | Purpose | Exposed | Description |
|--------|---------|---------|-------------|
| 0 | Coordinator gRPC | Yes | Angzarr sidecar coordinator |
| 1 | REST Proxy | Optional | REST → gRPC proxy |
| 2 | Coordinator Debug | No | Sidecar diagnostics |
| 3 | Client Logic | No | Internal sidecar-to-logic |
| 4 | Client Debug | No | Logic diagnostics |
| 5-8 | Reserved | No | Future use |
| 9 | Control/Meta UI | Optional | Admin UI, metrics |

---

## Port Ranges by Language

Each language gets a distinct range for concurrent local development:

| Language | Base Range | Aggregates | Sagas | Projectors |
|----------|------------|------------|-------|------------|
| Rust | 50050-50199 | 50050-50109 | 50110-50139 | 50140-50159 |
| Go | 50200-50349 | 50200-50259 | 50260-50289 | 50290-50309 |
| Python | 50400-50549 | 50400-50459 | 50460-50489 | 50490-50509 |
| Java | 50550-50699 | 50550-50609 | 50610-50639 | 50640-50659 |
| C# | 50700-50849 | 50700-50759 | 50760-50789 | 50790-50809 |
| C++ | 50850-50999 | 50850-50909 | 50910-50939 | 50940-50959 |

---

## Example: Rust Aggregates

| Service | Base | Coordinator | REST | Debug | Logic |
|---------|------|-------------|------|-------|-------|
| Player | 50050 | 50050 | 50051 | 50052 | 50053 |
| Table | 50060 | 50060 | 50061 | 50062 | 50063 |
| Hand | 50070 | 50070 | 50071 | 50072 | 50073 |

---

## Design Rationale

### Why ten ports?

1. **Coordinator (offset 0)** — Primary gRPC endpoint. Always exposed.
2. **REST Proxy (offset 1)** — Optional HTTP/REST for non-gRPC clients.
3. **Coordinator Debug (offset 2)** — Health checks, metrics for sidecar.
4. **Client Logic (offset 3)** — Internal sidecar-to-logic. Never exposed externally.
5. **Client Debug (offset 4)** — Logic debugging during development.
6. **Reserved (5-8)** — Future expansion without restructuring.
7. **Control UI (offset 9)** — Optional admin interface.

### Why separate ranges per language?

- Run all six implementations simultaneously for comparison testing
- Prevent port conflicts during local development
- Clear ownership when debugging multi-language deployments

---

## Kubernetes Considerations

In Kubernetes, container ports are typically remapped:

```yaml
# values.yaml
applications:
  business:
    - name: player
      ports:
        coordinator: 50050   # Exposed via Service
        rest: 50051          # Exposed if REST needed
        debug: 50052         # Exposed only if needed
        # Logic port (50053) stays internal to pod
```

The mesh/ingress routes to the coordinator port (offset 0). Internal sidecar-to-logic communication uses localhost within the pod.

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | varies | Coordinator gRPC port (offset 0) |
| `REST_PORT` | PORT+1 | REST proxy port (offset 1) |
| `DEBUG_PORT` | PORT+2 | Debug endpoint port (offset 2) |
| `TARGET_PORT` | PORT+3 | Client logic port (offset 3) |

---

## Next Steps

- **[Components](/components/aggregate)** — Sidecar architecture
- **[Infrastructure](/operations/infrastructure)** — Helm deployment
