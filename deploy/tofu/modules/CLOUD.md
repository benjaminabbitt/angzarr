# Angzarr Cloud Deployment Guide

Reference documentation for deploying Angzarr across cloud providers.

## Supported Cloud Runtimes

### GCP Cloud Run

**Status:** Supported via `domain/` module

Cloud Run provides serverless container execution with automatic scaling. Angzarr uses Cloud Run's multi-container (sidecar) feature to deploy the grpc-gateway, coordinator, and logic containers together.

**Key Features:**
- Zero-to-N autoscaling with configurable min/max instances
- Session affinity for aggregate routing
- VPC connector support for private networking
- Gen2 execution environment for improved performance
- Secret Manager integration

**Limitations:**
- Cold start latency (mitigate with min_instances > 0)
- 60-minute max request timeout
- No persistent local storage

### AWS Fargate

**Status:** Supported via `fargate-domain/` module

Fargate provides serverless container execution on ECS. Each domain component runs as an ECS service with a task definition containing the sidecar containers.

**Key Features:**
- Seamless integration with AWS ecosystem (ALB, Cloud Map, Secrets Manager)
- VPC networking with security groups
- CloudWatch logging
- Service discovery via Cloud Map DNS

**Limitations:**
- Minimum 1 task always running (no scale-to-zero)
- Task startup time ~30-60s
- Fixed CPU/memory configurations

### Kubernetes (EKS/GKE)

**Status:** Supported via `eks-domain/` and `gke-domain/` modules

Standard Kubernetes deployments with Horizontal Pod Autoscaler. Best for teams with existing Kubernetes expertise and infrastructure.

**Key Features:**
- Full Kubernetes ecosystem compatibility
- Horizontal Pod Autoscaler for CPU-based scaling
- Native service discovery via kube-dns
- Pod affinity/anti-affinity rules
- GKE: Workload Identity for secure GCP API access
- EKS: IAM Roles for Service Accounts (IRSA)

**Limitations:**
- Cluster management overhead
- No scale-to-zero without additional tooling (KEDA)

### AWS Lambda

**Status:** Not Supported

Lambda's execution model is incompatible with Angzarr's architecture:

1. **Sidecar Pattern**: Angzarr requires 3-4 containers running together (grpc-gateway, coordinator, logic, optional upcaster). Lambda functions run as single isolated containers.

2. **Long-Running Connections**: Coordinators maintain persistent connections to event buses and storage backends. Lambda's 15-minute max execution and cold starts break connection pooling.

3. **Session Affinity**: Aggregates require sticky routing to maintain in-memory state during command processing. Lambda provides no session affinity.

4. **gRPC**: The coordinator uses gRPC for internal communication. While Lambda supports gRPC, the sidecar architecture cannot be replicated.

**Alternatives:**
- Use Fargate for serverless containers on AWS
- Use EKS for Kubernetes on AWS
- For event-driven Lambda integration, deploy Lambda functions that call Angzarr aggregates via the REST API

### Azure Container Apps

**Status:** Planned (not yet implemented)

Container Apps supports the sidecar pattern and could work similarly to Cloud Run. Key features needed:
- Multi-container pod support ✓
- Session affinity ✓
- Scale-to-zero ✓
- Dapr integration (optional)

### Azure Kubernetes Service (AKS)

**Status:** Planned (not yet implemented)

Would use the same Kubernetes patterns as EKS/GKE. Implementation would be nearly identical to `eks-domain/` with Azure-specific features:
- Azure AD Workload Identity
- Azure Key Vault CSI driver

### Fly.io

**Status:** Under consideration

Fly.io supports multi-process apps and could potentially work with Angzarr. Investigation needed for:
- Sidecar container support
- Session affinity / sticky sessions
- gRPC support
- Scaling behavior

## Module Selection Guide

| Requirement | Recommended Module |
|-------------|-------------------|
| Minimal ops, GCP | `domain/` (Cloud Run) |
| Minimal ops, AWS | `fargate-domain/` |
| Existing K8s, GCP | `gke-domain/` |
| Existing K8s, AWS | `eks-domain/` |
| Scale-to-zero | `domain/` (Cloud Run) |
| Complex networking | `eks-domain/` or `gke-domain/` |
| Multi-cloud | Use K8s modules with consistent config |

## Adding New Provider Support

To add support for a new cloud provider:

1. Create `{provider}-domain/` directory
2. Copy `variables_business.tf` from an existing module (identical across all)
3. Create `variables_operational.tf` with provider-native patterns
4. Implement `main.tf` with the sidecar container pattern
5. Create `outputs.tf` matching the standard output structure
6. Add validation tests

The business config must remain identical across all providers. Only operational config should differ.
