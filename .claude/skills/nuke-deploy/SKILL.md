---
name: nuke-deploy
description: Tear down, rebuild from scratch, and redeploy to the Kind cluster
allowed-tools: Bash(just *)
---

# Nuke Deploy

Destroy the existing deployment, bust all caches, rebuild everything from scratch, and redeploy to the Kind cluster.

Run: `just nuke-deploy`

After completion:
1. Show the pod status output
2. Report any pods that are not Running/Ready
3. If the gateway health check failed, suggest checking logs with `just examples k8s logs gateway`
