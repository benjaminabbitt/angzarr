# Service Mesh Integration

Angzarr provides optional service mesh integration via separate Helm charts. Each mesh has its own chart with no runtime abstractions.

## Available Mesh Charts

| Chart | Description |
|-------|-------------|
| `angzarr-mesh-istio` | Istio service mesh integration |
| `angzarr-mesh-linkerd` | Linkerd service mesh integration |

## Installation

### Prerequisites

1. Angzarr core chart deployed
2. Service mesh installed in cluster

### Deploy with Istio

```bash
# Install Istio (if not already installed)
istioctl install --set profile=default

# Label namespace for sidecar injection
kubectl label namespace angzarr istio-injection=enabled

# Deploy angzarr core
helm install angzarr ./deploy/helm/angzarr -n angzarr

# Deploy Istio mesh integration
helm install angzarr-mesh ./deploy/helm/angzarr-mesh-istio -n angzarr
```

### Deploy with Linkerd

```bash
# Install Linkerd (if not already installed)
linkerd install --crds | kubectl apply -f -
linkerd install | kubectl apply -f -

# Inject Linkerd proxy
kubectl get deploy -n angzarr -o yaml | linkerd inject - | kubectl apply -f -

# Deploy angzarr core
helm install angzarr ./deploy/helm/angzarr -n angzarr

# Deploy Linkerd mesh integration
helm install angzarr-mesh ./deploy/helm/angzarr-mesh-linkerd -n angzarr
```

## Routing

Both mesh charts route traffic to Angzarr services using a header-based strategy:

### Aggregate Coordinators

Clients include `x-angzarr-domain` header to route to the correct aggregate:

```
x-angzarr-domain: order     → angzarr-order:1310
x-angzarr-domain: inventory → angzarr-inventory:1310
x-angzarr-domain: fulfillment → angzarr-fulfillment:1310
```

### EventStream

Stream traffic routes by service method:

```
/angzarr.EventStream/* → angzarr-stream:1340
```

## Istio Configuration

### values.yaml

```yaml
# Namespace where angzarr is deployed
angzarrNamespace: angzarr

# Gateway configuration
gateway:
  selector: istio-ingressgateway
  hosts:
    - "angzarr.example.com"
  port: 443
  tls:
    enabled: true
    mode: SIMPLE
    credentialName: angzarr-tls

# Aggregate domains
aggregates:
  domains:
    - name: order
      service: angzarr-order
      port: 1310
    - name: inventory
      service: angzarr-inventory
      port: 1310

# mTLS
mtls:
  enabled: true
  mode: STRICT

# Traffic policy
trafficPolicy:
  timeout: 30s
  retries:
    attempts: 3
    perTryTimeout: 5s
    retryOn: "connect-failure,refused-stream,unavailable"
  outlierDetection:
    enabled: true
    consecutiveGatewayErrors: 5
    interval: 30s
    baseEjectionTime: 30s
```

### Resources Created

| Resource | Purpose |
|----------|---------|
| `Gateway` | Ingress gateway for gRPC traffic |
| `VirtualService` | Routing rules for aggregates and stream |
| `DestinationRule` | Connection pool, circuit breaker, mTLS |
| `PeerAuthentication` | Namespace mTLS policy |

## Linkerd Configuration

### values.yaml

```yaml
# Namespace where angzarr is deployed
angzarrNamespace: angzarr

# Aggregate domains
aggregates:
  domains:
    - name: order
      service: angzarr-order
      port: 1310

# Authorization
authorization:
  mtls: true
  allowUnauthenticated: true  # For ingress

# Gateway API parent reference
httpRoute:
  parentRefs:
    - name: linkerd-gateway
      namespace: linkerd
      sectionName: grpc
```

### Resources Created

| Resource | Purpose |
|----------|---------|
| `Server` | gRPC protocol detection for each service |
| `GRPCRoute` | Gateway API routing rules |
| `AuthorizationPolicy` | mTLS enforcement per service |
| `MeshTLSAuthentication` | Mesh identity validation |
| `NetworkAuthentication` | Unauthenticated ingress access |

## Adding New Domains

When adding a new aggregate domain, update the mesh chart values:

```yaml
aggregates:
  domains:
    - name: order
      service: angzarr-order
      port: 1310
    - name: newdomain          # Add new domain
      service: angzarr-newdomain
      port: 1310
```

Then upgrade the mesh chart:

```bash
helm upgrade angzarr-mesh ./deploy/helm/angzarr-mesh-istio -n angzarr
```

## Observability Integration

Service meshes provide additional observability:

### Istio
- Traces automatically propagate via Envoy
- Metrics exported to Prometheus via Istio telemetry
- See [Observability](./observability.md) for collector configuration

### Linkerd
- Automatic mTLS and request-level metrics
- Linkerd-viz dashboard for service topology
- Integrates with Grafana via Linkerd dashboards

## Troubleshooting

### Traffic not reaching services

1. Verify mesh sidecar is injected:
   ```bash
   kubectl get pods -n angzarr -o jsonpath='{.items[*].spec.containers[*].name}' | tr ' ' '\n' | grep -E 'istio-proxy|linkerd-proxy'
   ```

2. Check routing configuration:
   ```bash
   # Istio
   istioctl analyze -n angzarr

   # Linkerd
   linkerd check --proxy -n angzarr
   ```

3. Verify header is being sent:
   ```bash
   grpcurl -H "x-angzarr-domain: order" ...
   ```

### mTLS issues

1. Check peer authentication status:
   ```bash
   # Istio
   kubectl get peerauthentication -n angzarr

   # Linkerd
   linkerd viz authz -n angzarr
   ```

2. Temporarily disable mTLS for debugging:
   ```yaml
   # Istio
   mtls:
     mode: PERMISSIVE

   # Linkerd
   authorization:
     allowUnauthenticated: true
   ```
