# Common Problems

## Kind + Rootless Podman: Kubelet Fails to Start

**Symptom:**
```
ERROR: failed to create cluster: failed to init node with kubeadm
[kubelet-check] The kubelet is not healthy after 4m0s
error: dial tcp 127.0.0.1:10248: connect: connection refused
```

**Cause:**
On cgroups v2 systems, rootless containers need explicit cgroup delegation to manage nested containers. Kind's kubelet requires full cgroup access to manage pod resources.

**Fix (one-time, requires sudo):**

```bash
# Create cgroup delegation config
sudo mkdir -p /etc/systemd/system/user@.service.d
sudo tee /etc/systemd/system/user@.service.d/delegate.conf << 'EOF'
[Service]
Delegate=cpu cpuset io memory pids
EOF

# Reload systemd
sudo systemctl daemon-reload
```

Log out and back in (or reboot).

**Verify fix:**
```bash
cat /sys/fs/cgroup/user.slice/user-$(id -u).slice/user@$(id -u).service/cgroup.controllers
# Should show: cpuset cpu io memory pids
```

Then retry:
```bash
just kind-create
```

---

## Podman: Port Already in Use

**Symptom:**
```
Error: rootlessport listen tcp 0.0.0.0:5672: bind: address already in use
```

**Cause:**
Podman's rootlesskit process holds ports even after containers stop.

**Fix:**
```bash
# Stop all containers
podman stop -a
podman rm -a

# If ports still held, kill rootlesskit
pkill -9 rootlesskit

# Verify ports are free
ss -tlnp | grep -E ':(5672|6379|50051)'
```

---

## Kind: Cluster Already Exists

**Symptom:**
```
ERROR: failed to create cluster: node(s) already exist for a cluster with the name "angzarr"
```

**Fix:**
```bash
kind delete cluster --name angzarr
just kind-create
```

