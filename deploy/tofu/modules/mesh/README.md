# Service Mesh Module

Deploys Linkerd or Istio service mesh for L7 gRPC load balancing and mTLS.

## Usage

```hcl
module "mesh" {
  source = "github.com/benjaminabbitt/angzarr//deploy/tofu/modules/mesh?ref=v0.1.0"

  type             = "linkerd"  # or "istio"
  namespace        = "angzarr"
  inject_namespace = true

  # Linkerd requires trust anchor and issuer certs
  linkerd_trust_anchor_pem = file("ca.crt")
  linkerd_issuer_cert_pem  = file("issuer.crt")
  linkerd_issuer_key_pem   = file("issuer.key")
}
```

## Inputs

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|----------|
| type | Mesh type: `linkerd` or `istio` | string | `"linkerd"` | no |
| namespace | Target namespace for injection | string | `"angzarr"` | no |
| inject_namespace | Annotate namespace for sidecar injection | bool | `true` | no |
| linkerd_chart_version | Linkerd Helm chart version | string | `"1.16.0"` | no |
| istio_chart_version | Istio Helm chart version | string | `"1.20.0"` | no |
| linkerd_trust_anchor_pem | Linkerd trust anchor certificate (PEM) | string | `""` | Linkerd |
| linkerd_issuer_cert_pem | Linkerd issuer certificate (PEM) | string | `""` | Linkerd |
| linkerd_issuer_key_pem | Linkerd issuer key (PEM) | string | `""` | Linkerd |
| linkerd_run_as_root | Run Linkerd init as root | bool | `false` | no |
| proxy_resources | Sidecar proxy resource requests/limits | object | see below | no |
| control_plane_resources | Control plane resource requests/limits | object | see below | no |

### Default Resources

```hcl
proxy_resources = {
  requests = { memory = "64Mi", cpu = "10m" }
  limits   = { memory = "256Mi", cpu = "1000m" }
}

control_plane_resources = {
  requests = { memory = "256Mi", cpu = "100m" }
  limits   = { memory = "1Gi", cpu = "1000m" }
}
```

## Outputs

| Name | Description |
|------|-------------|
| type | Mesh type deployed |
| namespace | Mesh control plane namespace |

## Generating Linkerd Certificates

```bash
# Generate CA
step certificate create root.linkerd.cluster.local ca.crt ca.key \
  --profile root-ca --no-password --insecure

# Generate issuer
step certificate create identity.linkerd.cluster.local issuer.crt issuer.key \
  --profile intermediate-ca --not-after 8760h --no-password --insecure \
  --ca ca.crt --ca-key ca.key
```

## Requirements

| Name | Version |
|------|---------|
| terraform/opentofu | >= 1.0 |
| helm | ~> 2.0 |
| kubernetes | ~> 2.0 |
