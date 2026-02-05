# Kind/K8s Deployment Problems & Workarounds

This document tracks issues encountered when deploying to kind and their solutions.

## Setup

Cluster created with:
```bash
./scripts/kind-with-registry.py create
```

---

## Issues

### 1. Registry Container Exists but Not on Kind Network

**Problem:** When re-creating a cluster, the registry container may exist from a previous run but not be connected to the new `kind` network.

**Error:**
```
Error: name "kind-registry" is in use: container already exists
```

**Solution:**
```bash
# Remove old registry and recreate on kind network
podman rm kind-registry
podman run -d --restart=always --network kind \
  -p 127.0.0.1:5001:5000 \
  -v kind-registry-data:/var/lib/registry \
  -e REGISTRY_STORAGE_DELETE_ENABLED=true \
  --name kind-registry docker.io/library/registry:2
```

**Root Cause:** The `kind-with-registry.py` script tries to handle this case but the container was in `Exited` state rather than running. The script's `registry_running()` check fails, and the subsequent `podman run` fails because the container name exists.

**Status:** Fixed. Script now uses `podman rm -f` to handle containers in any state, and retries with recreate if `podman start` fails.

---

### 2. MongoDB Lock File on Redeployment

**Problem:** When redeploying, MongoDB may fail with a lock file error if the previous pod's data wasn't cleaned up properly.

**Error:**
```
DBPathInUse: Unable to lock the lock file: /bitnami/mongodb/data/db/mongod.lock (Resource temporarily unavailable).
Another mongod instance is already running on the /bitnami/mongodb/data/db directory
```

**Solution:**
```bash
# Delete the MongoDB deployment to release the lock
kubectl delete deployment angzarr-db-mongodb -n angzarr
# Then redeploy
skaffold run
```

**Root Cause:** The emptyDir volume can persist across pod restarts within the same deployment revision, causing lock file conflicts.

**Attempted Fix:** Init container to remove `mongod.lock` on startup was tried but caused rolling update failures with bitnami/mongodb chart (exit code 100 during initialization).

**Current Workaround:** Delete the MongoDB deployment before redeploying:
```bash
kubectl delete deployment angzarr-db-mongodb -n angzarr
skaffold run
```

---

### 3. POD_NAME/POD_NAMESPACE Missing in Helm Templates

**Problem:** Coordinator sidecars (saga, projector, process-manager) were not writing descriptor annotations to their pods because the POD_NAME and POD_NAMESPACE environment variables were only configured for aggregate sidecars.

**Symptoms:**
- Aggregate pods had `angzarr.io/descriptor` annotations
- Saga, projector, and PM pods had no descriptor annotations
- Topology service only discovered aggregates

**Solution:** Add the following env vars to all sidecar containers in `deploy/helm/angzarr/templates/deployment.yaml`:
```yaml
- name: POD_NAMESPACE
  valueFrom:
    fieldRef:
      fieldPath: metadata.namespace
- name: POD_NAME
  valueFrom:
    fieldRef:
      fieldPath: metadata.name
```

**Files Modified:**
- Saga sidecar section (around line 560)
- Projector sidecar section (around line 365)
- Process Manager sidecar section (around line 725)
- Topology container section (around line 1165)

**Status:** Fixed. Verified all sidecars (aggregate, projector, saga, process-manager) have both POD_NAME and POD_NAMESPACE. Topology only needs POD_NAMESPACE (for watching pods).

---

### 4. Topology Nodes Deleted During Rolling Updates (BUG)

**Problem:** The K8s pod watcher deletes topology nodes when ANY pod with a matching descriptor name is deleted. During rolling updates, the old pod is deleted after the new pod is created, but the node deletion uses `descriptor.name` (e.g., "order") not `pod.name` (e.g., "angzarr-agg-order-rs-abc123"). This causes the node to be deleted even though a replacement pod exists.

**Symptoms:**
- Topology shows only 1 node (the last one to be discovered without a subsequent delete)
- Logs show: "Discovered component... order" followed by "Removing component... order"
- Graph data is nearly empty after deployments

**Root Cause:** The `handle_pod_delete` function in `k8s_watcher.rs` deletes by `node_id` (descriptor name) rather than tracking which pods contribute to each node:
```rust
// Current (buggy) behavior
let node_id = descriptor.name; // e.g., "order"
self.store.delete_node(&node_id).await  // Deletes the node!
```

**Solution:** Reference counting implemented in `k8s_watcher.rs`:
- Added `node_pods: RwLock<HashMap<String, HashSet<String>>>` to track pod→node mapping
- `handle_pod_apply`: Tracks each pod as contributing to its node
- `handle_pod_delete`: Only deletes node when ALL contributing pods are gone
- `Event::Init`: Clears tracking map when K8s re-syncs

**Status:** Fixed.

---

### 5. Skaffold SHA Pinning Prevents Image Updates

**Problem:** Skaffold pins image references with SHA digests in deployments (e.g., `localhost:5001/angzarr-topology:latest@sha256:abc123...`). This prevents `imagePullPolicy: Always` from pulling newer images with the same tag.

**Symptoms:**
- Rebuilding an image and pushing to registry doesn't update running pods
- `kubectl rollout restart` still uses the old cached image
- Even `imagePullPolicy: Always` pulls the same old SHA

**Root Cause:** Skaffold writes the full image reference including SHA digest to the deployment spec. K8s then always pulls that exact SHA, ignoring the `latest` tag.

**Solution:**
```bash
# Option 1: Update image to remove SHA pinning
kubectl set image deployment/<name> -n <ns> <container>=<repo>:<tag>

# Option 2: Use skaffold to redeploy (will update SHA)
skaffold run

# Option 3: Rebuild with --no-cache to get new SHA
podman build --no-cache -t <image>
```

**Prevention:** Consider using explicit version tags instead of `:latest` in production, or use a deployment strategy that doesn't pin SHAs.

**Partial Fix:** Changed `skaffold.yaml` tagPolicy from `sha256` to `gitCommit` with `dev-` prefix. This only helps when using skaffold - see issue #9 for manual build caching.

---

### 9. Kind Node Image Cache Prevents Manual Image Updates

**Problem:** When manually building and pushing images (not via skaffold), Kind nodes cache images at the containerd level. Even pushing a new image with the same tag to the registry doesn't update pods because:
1. `imagePullPolicy: IfNotPresent` won't re-pull if tag exists locally
2. The node's containerd cache serves the old image
3. Even unique tags may resolve to cached layers

**Symptoms:**
- `podman build && podman push` completes successfully
- Registry shows new image digest (verified via `curl` to registry API)
- Pod continues running old code
- `kubectl rollout restart` creates new pod with OLD image
- Pod's `imageID` shows different SHA than registry manifest

**Example:**
```bash
# Registry has new image
$ curl -s -H "Accept: application/vnd.oci.image.manifest.v1+json" \
    http://localhost:5001/v2/agg-order-rs/manifests/v2 | jq -r '.config.digest'
sha256:e57bee30ad2b...

# But pod is running old image
$ kubectl get pod -o jsonpath='{.status.containerStatuses[0].imageID}'
localhost:5001/agg-order-rs@sha256:ade2214ae849...  # DIFFERENT!
```

**Root Cause:** Kind nodes use containerd which caches images by tag. When you push a new image with an existing tag:
1. Registry accepts the new manifest
2. Node still has old image cached for that tag
3. `IfNotPresent` sees tag exists locally → no pull
4. Even `Always` may not help if containerd resolves tag from cache

**Solutions (in order of reliability):**

```bash
# Option 1: Use unique tag per build (RECOMMENDED)
TAG="dev-$(date +%s)"
podman build -t localhost:5001/myimage:$TAG ...
podman push localhost:5001/myimage:$TAG
kubectl set image deployment/myapp container=localhost:5001/myimage:$TAG

# Option 2: Clear node's image cache for specific image
docker exec <kind-node> crictl rmi localhost:5001/myimage:mytag
kubectl delete pod -l app=myapp  # Force new pod to pull

# Option 3: Use imagePullPolicy: Always + delete pod
# (Less reliable - containerd may still serve cached)
kubectl delete pod -l app=myapp

# Option 4: Nuclear option - clear all node images
docker exec <kind-node> crictl rmi --all
```

**Best Practice for Development:**
1. Always use unique tags for manual builds (timestamp or git SHA)
2. Use `kubectl set image` to update deployment with new tag
3. Verify correct image is running: compare pod's `imageID` with registry manifest
4. When in doubt, clear node cache with `crictl rmi`

**Why Skaffold Works:** Skaffold uses content-addressable tags (git commit SHA) so each build gets a unique tag, avoiding cache collisions entirely.

**Recommended Solution:** Use skaffold exclusively for all deployments. Never bypass with manual `podman build` + `helm upgrade` workflows.

```bash
# ALWAYS use skaffold for deployments
skaffold run -f examples/rust/skaffold.yaml

# NEVER do this (will hit cache issues):
podman build -t localhost:5001/myimage:latest ...
podman push localhost:5001/myimage:latest
helm upgrade ...
```

**Status:** Resolved by policy - use skaffold for all deployments.

---

### 7. Port-Forward Management in Scripted Environments

**Problem:** Running `kubectl port-forward` in background with `&` in shell scripts can leave orphaned processes, and combining with `sleep` may fail due to environment differences (e.g., `sleep: invalid option -- 's'`).

**Symptoms:**
- Multiple port-forward processes accumulate
- Port conflicts (`bind: address already in use`)
- Scripts fail with sleep syntax errors in some environments

**Solution:**
```bash
# Kill existing port-forwards before starting new ones
pkill -f "port-forward.*<port>" 2>/dev/null || true

# Run port-forward in background, then query in separate command
kubectl port-forward -n <ns> svc/<service> <local>:<remote> &
# Wait separately, then query
curl -s http://localhost:<local>/endpoint

# Or use --address to avoid IPv4/IPv6 conflicts
kubectl port-forward --address 127.0.0.1 -n <ns> svc/<service> <port>:<port>
```

**Best Practice:** Use unique local ports (e.g., 19099, 29099) to avoid conflicts with existing port-forwards, and clean up after testing.

**Status:** Fixed. Added `just port-forward-cleanup`, `just port-forward-gateway`, `just port-forward-topology`, and `just port-forward-grafana` targets.

---

### 8. Grafana NodeGraph API Datasource Plugin Proxy Error

**Problem:** The `hamedkarbasi93-nodegraphapi-datasource` plugin returns 502 Bad Gateway with error `unsupported protocol scheme ""` even though the datasource URL is correctly configured.

**Symptoms:**
- Direct calls to topology service work (from inside Grafana pod)
- Grafana datasource shows correct URL in API response
- Proxy requests fail with empty protocol scheme error

**Root Cause:** Unknown - possible plugin bug in version 1.0.1. The plugin may be reading the URL from an unexpected location.

**Workarounds:**
1. Access topology API directly via port-forward instead of through Grafana proxy
2. Consider using Infinity plugin instead (the nodegraph plugin is archived/unmaintained)

**Status:** Under investigation.

---

### 6. Podman Build Cache With Mounted Source Directories

**Problem:** Podman build with `--mount=type=cache` for cargo registry/target doesn't invalidate when source files change, causing stale binaries.

**Symptoms:**
- Editing Rust source files doesn't trigger recompilation
- Old code runs despite changes being visible in git

**Root Cause:** Build cache mounts persist between builds. The `COPY src/` layer hash doesn't change if files have same metadata, and cargo's incremental compilation doesn't see the changes.

**Solution:**
```bash
# Force full rebuild without cache
podman build --no-cache ...

# Or invalidate cargo cache specifically
rm -rf ~/.cargo/registry/cache
```

**Status:** Fixed. Containerfile refactored to use two-stage build:
1. `builder-*-deps` stage: Builds dependencies with stub source (cached until Cargo.toml/Cargo.lock change)
2. `builder-*` stage: Copies real source and rebuilds (invalidates when src/ changes)

This uses Docker layer caching instead of mount-based caching, ensuring source changes trigger rebuilds.

---
