# gRPC debugging and testing helpers for angzarr
# Usage: just grpc <command>

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

# Show available commands
default:
    @just --list

# === Health Checks ===

# Check gRPC health of all core services
health-check:
    @uv run "{{TOP}}/scripts/wait-for-grpc-health.py" --timeout 10 --interval 1 \
        localhost:50051 localhost:50052 localhost:50053 localhost:50054

# Check gRPC health of command handler only
health-command:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50051 grpc.health.v1.Health/Check

# Check gRPC health of query service only
health-query:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50052 grpc.health.v1.Health/Check

# Check gRPC health of gateway only
health-gateway:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50053 grpc.health.v1.Health/Check

# Check gRPC health of stream only
health-stream:
    @grpcurl -plaintext -d '{"service": ""}' localhost:50054 grpc.health.v1.Health/Check

# === Service Discovery ===

# List available gRPC services via command handler
list-command:
    grpcurl -plaintext localhost:50051 list

# List available gRPC services via gateway
list-gateway:
    grpcurl -plaintext localhost:50053 list

# List available gRPC services via stream
list-stream:
    grpcurl -plaintext localhost:50054 list

# Describe BusinessCoordinator service
describe-command:
    grpcurl -plaintext localhost:50051 describe angzarr.BusinessCoordinator

# Describe CommandProxy service
describe-gateway:
    grpcurl -plaintext localhost:50053 describe angzarr.CommandProxy

# Describe EventStream service
describe-stream:
    grpcurl -plaintext localhost:50054 describe angzarr.EventStream

# === Example Commands ===

# Send a command via command handler
example-command DOMAIN AGGREGATE_ID:
    @echo "Sending command to {{DOMAIN}}/{{AGGREGATE_ID}}..."
    grpcurl -plaintext -d '{"cover": {"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}}, "pages": []}' \
        localhost:50051 angzarr.BusinessCoordinator/Handle

# Send a command via gateway with streaming response
example-gateway DOMAIN AGGREGATE_ID:
    @echo "Sending command via gateway to {{DOMAIN}}/{{AGGREGATE_ID}} with streaming..."
    grpcurl -plaintext -d '{"cover": {"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}}, "pages": [], "correlation_id": ""}' \
        localhost:50053 angzarr.CommandProxy/Execute

# Subscribe to events by correlation ID
subscribe-stream CORRELATION_ID:
    @echo "Subscribing to events with correlation ID {{CORRELATION_ID}}..."
    grpcurl -plaintext -d '{"correlation_id": "{{CORRELATION_ID}}"}' \
        localhost:50054 angzarr.EventStream/Subscribe

# Query events for an aggregate
query-events DOMAIN AGGREGATE_ID:
    @echo "Querying events for {{DOMAIN}}/{{AGGREGATE_ID}}..."
    grpcurl -plaintext -d '{"domain": "{{DOMAIN}}", "root": {"value": "{{AGGREGATE_ID}}"}, "lower_bound": 0, "upper_bound": 0}' \
        localhost:50052 angzarr.EventQuery/GetEvents
