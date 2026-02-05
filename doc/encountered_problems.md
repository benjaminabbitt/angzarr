# Encountered Problems & Solutions

This document tracks issues encountered during development and their solutions.

## Local K8s Development (Kind)

Cluster created with:
```bash
./scripts/kind-with-registry.py create
```

---

## Known Issues (Unfixed)

### MongoDB Lock File on Redeployment

**Problem:** When redeploying, MongoDB may fail with a lock file error if the previous pod's data wasn't cleaned up properly.

**Error:**
```
DBPathInUse: Unable to lock the lock file: /bitnami/mongodb/data/db/mongod.lock (Resource temporarily unavailable).
Another mongod instance is already running on the /bitnami/mongodb/data/db directory
```

**Root Cause:** The emptyDir volume can persist across pod restarts within the same deployment revision, causing lock file conflicts.

**Attempted Fix:** Init container to remove `mongod.lock` on startup was tried but caused rolling update failures with bitnami/mongodb chart (exit code 100 during initialization).

**Workaround:** Delete the MongoDB deployment before redeploying:
```bash
kubectl delete deployment angzarr-db-mongodb -n angzarr
skaffold run
```

---

### Grafana NodeGraph API Datasource Plugin Proxy Error

**Problem:** The `hamedkarbasi93-nodegraphapi-datasource` plugin returns 502 Bad Gateway with error `unsupported protocol scheme ""` even though the datasource URL is correctly configured.

**Symptoms:**
- Direct calls to topology service work (from inside Grafana pod)
- Grafana datasource shows correct URL in API response
- Proxy requests fail with empty protocol scheme error

**Root Cause:** Unknown - possible plugin bug in version 1.0.1. The plugin may be reading the URL from an unexpected location.

**Workarounds:**
1. Access topology API directly via port-forward instead of through Grafana proxy
2. Consider using Infinity plugin instead (the nodegraph plugin is archived/unmaintained)

---

## Resolved Issues

### Kind Node Image Cache (Use Skaffold)

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

**Root Cause:** Kind nodes use containerd which caches images by tag. When you push a new image with an existing tag:
1. Registry accepts the new manifest
2. Node still has old image cached for that tag
3. `IfNotPresent` sees tag exists locally → no pull
4. Even `Always` may not help if containerd resolves tag from cache

**Solution:** Use skaffold exclusively for all deployments. Skaffold uses content-addressable tags (git commit SHA) so each build gets a unique tag, avoiding cache collisions entirely.

```bash
# ALWAYS use skaffold for deployments
skaffold run -f examples/rust/skaffold.yaml

# NEVER do this (will hit cache issues):
podman build -t localhost:5001/myimage:latest ...
podman push localhost:5001/myimage:latest
helm upgrade ...
```

---

### Registry Container Exists but Not on Kind Network

**Problem:** When re-creating a cluster, the registry container may exist from a previous run but not be connected to the new `kind` network.

**Error:**
```
Error: name "kind-registry" is in use: container already exists
```

**Root Cause:** The `kind-with-registry.py` script tries to handle this case but the container was in `Exited` state rather than running. The script's `registry_running()` check fails, and the subsequent `podman run` fails because the container name exists.

**Solution:** Script now uses `podman rm -f` to handle containers in any state, and retries with recreate if `podman start` fails.

---

### POD_NAME/POD_NAMESPACE Missing in Helm Templates

**Problem:** Coordinator sidecars (saga, projector, process-manager) were not writing descriptor annotations to their pods because the POD_NAME and POD_NAMESPACE environment variables were only configured for aggregate sidecars.

**Symptoms:**
- Aggregate pods had `angzarr.io/descriptor` annotations
- Saga, projector, and PM pods had no descriptor annotations
- Topology service only discovered aggregates

**Solution:** Added env vars to all sidecar containers in `deploy/helm/angzarr/templates/deployment.yaml`:
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

---

### Topology Nodes Deleted During Rolling Updates

**Problem:** The K8s pod watcher deletes topology nodes when ANY pod with a matching descriptor name is deleted. During rolling updates, the old pod is deleted after the new pod is created, but the node deletion uses `descriptor.name` (e.g., "order") not `pod.name` (e.g., "angzarr-agg-order-rs-abc123"). This causes the node to be deleted even though a replacement pod exists.

**Symptoms:**
- Topology shows only 1 node (the last one to be discovered without a subsequent delete)
- Logs show: "Discovered component... order" followed by "Removing component... order"
- Graph data is nearly empty after deployments

**Solution:** Reference counting implemented in `k8s_watcher.rs`:
- Added `node_pods: RwLock<HashMap<String, HashSet<String>>>` to track pod→node mapping
- `handle_pod_apply`: Tracks each pod as contributing to its node
- `handle_pod_delete`: Only deletes node when ALL contributing pods are gone
- `Event::Init`: Clears tracking map when K8s re-syncs

---

### Port-Forward Management in Scripted Environments

**Problem:** Running `kubectl port-forward` in background with `&` in shell scripts can leave orphaned processes, and combining with `sleep` may fail due to environment differences.

**Symptoms:**
- Multiple port-forward processes accumulate
- Port conflicts (`bind: address already in use`)
- Scripts fail with sleep syntax errors in some environments

**Solution:** Added justfile targets for managed port-forwarding:
```bash
just port-forward-cleanup   # Kill all angzarr port-forwards
just port-forward-gateway   # Start gateway on 9084
just port-forward-topology  # Start topology on 9099
just port-forward-grafana   # Start grafana on 3000
```

---

### Podman Build Cache With Mounted Source Directories

**Problem:** Podman build with `--mount=type=cache` for cargo registry/target doesn't invalidate when source files change, causing stale binaries.

**Symptoms:**
- Editing Rust source files doesn't trigger recompilation
- Old code runs despite changes being visible in git

**Root Cause:** Build cache mounts persist between builds. The `COPY src/` layer hash doesn't change if files have same metadata, and cargo's incremental compilation doesn't see the changes.

**Solution:** Containerfile refactored to use two-stage build:
1. `builder-*-deps` stage: Builds dependencies with stub source (cached until Cargo.toml/Cargo.lock change)
2. `builder-*` stage: Copies real source and rebuilds (invalidates when src/ changes)

This uses Docker layer caching instead of mount-based caching, ensuring source changes trigger rebuilds.
