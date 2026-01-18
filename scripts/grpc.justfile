# gRPC debugging and testing helpers for angzarr
# Usage: just grpc <command>
#
# Port Scheme:
#   Gateway: 1350 (primary entry point - commands + queries)
#   Stream:  1340 (event streaming)
#   Aggregate sidecar: 1310 (internal)

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

# Show available commands
default:
    @just --list

# === Health Checks ===

# Check gRPC health of gateway
health-check:
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 10 --interval 1 \
        localhost:1350 localhost:1340

# Check gRPC health of gateway only
health-gateway:
    @grpcurl -plaintext -d '{"service": ""}' localhost:1350 grpc.health.v1.Health/Check

# Check gRPC health of stream only
health-stream:
    @grpcurl -plaintext -d '{"service": ""}' localhost:1340 grpc.health.v1.Health/Check

# === Service Discovery ===

# List available gRPC services via gateway
list:
    grpcurl -plaintext localhost:1350 list

# List available gRPC services via gateway (alias)
list-gateway:
    grpcurl -plaintext localhost:1350 list

# List available gRPC services via stream
list-stream:
    grpcurl -plaintext localhost:1340 list

# Describe CommandGateway service
describe-gateway:
    grpcurl -plaintext localhost:1350 describe angzarr.CommandGateway

# Describe EventQuery service
describe-query:
    grpcurl -plaintext localhost:1350 describe angzarr.EventQuery

# Describe EventStream service
describe-stream:
    grpcurl -plaintext localhost:1340 describe angzarr.EventStream

# === Example Commands ===

# Send a command via gateway
send-command DOMAIN AGGREGATE_ID:
    @echo "Sending command to {{DOMAIN}}/{{AGGREGATE_ID}}..."
    grpcurl -plaintext -d '{"cover": {"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}}, "pages": [], "correlation_id": ""}' \
        localhost:1350 angzarr.CommandGateway/Execute

# Query events for an aggregate
query-events DOMAIN AGGREGATE_ID:
    @echo "Querying events for {{DOMAIN}}/{{AGGREGATE_ID}}..."
    grpcurl -plaintext -d '{"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}, "lower_bound": 0, "upper_bound": 0}' \
        localhost:1350 angzarr.EventQuery/GetEvents

# Subscribe to events by correlation ID
subscribe CORRELATION_ID:
    @echo "Subscribing to events with correlation ID {{CORRELATION_ID}}..."
    grpcurl -plaintext -d '{"correlation_id": "{{CORRELATION_ID}}"}' \
        localhost:1340 angzarr.EventStream/Subscribe
