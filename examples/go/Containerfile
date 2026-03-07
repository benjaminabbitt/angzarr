# syntax=docker/dockerfile:1.4
# Go poker examples - optimized multi-stage build
# Build: podman build -t go-poker-player --target agg-player -f examples/go/Containerfile .
# Multi-arch: podman build --platform linux/amd64,linux/arm64 ...
# Context must be repo root for proto access
#
# Optimizations:
# 1. Shared deps-fetcher stage - go mod download runs once
# 2. Named cache IDs for Go module cache persistence
# 3. Distroless runtime - minimal attack surface
# 4. Multi-arch support (amd64 + arm64)

ARG GO_VERSION=1.23

# ============================================================================
# Base builder - common SDK
# ============================================================================
FROM docker.io/library/golang:${GO_VERSION}-alpine AS base

RUN apk add --no-cache ca-certificates git

WORKDIR /app

# ============================================================================
# Proto generator - generates Go proto files from .proto sources
# ============================================================================
FROM docker.io/bufbuild/buf:1.47.2 AS proto-gen

WORKDIR /app

# Copy proto files and buf config
COPY proto ./proto

WORKDIR /app/proto

# Generate Go proto files (output: ../client/go/proto/)
RUN buf generate --template buf.gen.go.yaml

# ============================================================================
# Deps fetcher - downloads ALL dependencies once
# ============================================================================
FROM base AS deps-fetcher

WORKDIR /app

# Copy client library (local dependency via go.mod replace directive)
# The replace directive is: replace ... => ../../client/go
# From /app/examples/go, that resolves to /app/client/go
COPY client/go ./client/go

# Copy generated proto files from proto-gen stage
# These are gitignored locally but needed for the build
COPY --from=proto-gen /app/client/go/proto ./client/go/proto

# Copy go.mod/go.sum for dependency resolution
COPY examples/go/go.mod examples/go/go.sum ./examples/go/

WORKDIR /app/examples/go

# Download dependencies with persistent cache
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    go mod download

# ============================================================================
# Aggregate builds - each builds a static binary
# ============================================================================
FROM deps-fetcher AS build-player
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./player/agg

FROM deps-fetcher AS build-table
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./table/agg

FROM deps-fetcher AS build-hand
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./hand/agg

# ============================================================================
# Saga builds
# ============================================================================
FROM deps-fetcher AS build-saga-table-hand
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./table/saga-hand

FROM deps-fetcher AS build-saga-table-player
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./table/saga-player

FROM deps-fetcher AS build-saga-hand-table
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./hand/saga-table

FROM deps-fetcher AS build-saga-hand-player
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./hand/saga-player

# ============================================================================
# Process Manager build
# ============================================================================
FROM deps-fetcher AS build-pmg-hand-flow
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./pmg-hand-flow

# ============================================================================
# Projector build
# ============================================================================
FROM deps-fetcher AS build-prj-output
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -ldflags="-s -w" -o /out/server ./prj-output

# ============================================================================
# Runtime base - distroless static (minimal, secure)
# ============================================================================
FROM gcr.io/distroless/static-debian12:nonroot AS runtime
WORKDIR /app
USER nonroot:nonroot

# ============================================================================
# Domain Aggregates
# ============================================================================
FROM runtime AS agg-player
COPY --from=build-player --chown=nonroot:nonroot /out/server ./server
ENV PORT=50201
EXPOSE 50201
ENTRYPOINT ["./server"]

FROM runtime AS agg-table
COPY --from=build-table --chown=nonroot:nonroot /out/server ./server
ENV PORT=50202
EXPOSE 50202
ENTRYPOINT ["./server"]

FROM runtime AS agg-hand
COPY --from=build-hand --chown=nonroot:nonroot /out/server ./server
ENV PORT=50203
EXPOSE 50203
ENTRYPOINT ["./server"]

# ============================================================================
# Sagas
# ============================================================================
FROM runtime AS saga-table-hand
COPY --from=build-saga-table-hand --chown=nonroot:nonroot /out/server ./server
ENV PORT=50211
EXPOSE 50211
ENTRYPOINT ["./server"]

FROM runtime AS saga-table-player
COPY --from=build-saga-table-player --chown=nonroot:nonroot /out/server ./server
ENV PORT=50212
EXPOSE 50212
ENTRYPOINT ["./server"]

FROM runtime AS saga-hand-table
COPY --from=build-saga-hand-table --chown=nonroot:nonroot /out/server ./server
ENV PORT=50213
EXPOSE 50213
ENTRYPOINT ["./server"]

FROM runtime AS saga-hand-player
COPY --from=build-saga-hand-player --chown=nonroot:nonroot /out/server ./server
ENV PORT=50214
EXPOSE 50214
ENTRYPOINT ["./server"]

# ============================================================================
# Process Manager
# ============================================================================
FROM runtime AS pmg-hand-flow
COPY --from=build-pmg-hand-flow --chown=nonroot:nonroot /out/server ./server
ENV PORT=50291
EXPOSE 50291
ENTRYPOINT ["./server"]

# ============================================================================
# Projector
# ============================================================================
FROM runtime AS prj-output
COPY --from=build-prj-output --chown=nonroot:nonroot /out/server ./server
ENV PORT=50290
EXPOSE 50290
ENTRYPOINT ["./server"]

# ============================================================================
# Debug builds - with debug symbols (no -ldflags stripping)
# ============================================================================
FROM deps-fetcher AS build-player-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./player/agg

FROM deps-fetcher AS build-table-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./table/agg

FROM deps-fetcher AS build-hand-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./hand/agg

FROM deps-fetcher AS build-saga-table-hand-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./table/saga-hand

FROM deps-fetcher AS build-saga-table-player-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./table/saga-player

FROM deps-fetcher AS build-saga-hand-table-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./hand/saga-table

FROM deps-fetcher AS build-saga-hand-player-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./hand/saga-player

FROM deps-fetcher AS build-pmg-hand-flow-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./pmg-hand-flow

FROM deps-fetcher AS build-prj-output-debug
WORKDIR /app/examples/go
COPY examples/go/ ./
RUN --mount=type=cache,id=go-mod-cache,target=/go/pkg/mod,sharing=locked \
    --mount=type=cache,id=go-build-cache,target=/root/.cache/go-build,sharing=locked \
    CGO_ENABLED=0 go build -gcflags="all=-N -l" -o /out/server ./prj-output

# ============================================================================
# Debug runtime - Alpine with Delve
# ============================================================================
FROM docker.io/library/golang:${GO_VERSION}-alpine AS runtime-debug
RUN apk add --no-cache ca-certificates && \
    go install github.com/go-delve/delve/cmd/dlv@latest
WORKDIR /app

# ============================================================================
# Debug Aggregates
# ============================================================================
FROM runtime-debug AS agg-player-debug
COPY --from=build-player-debug /out/server ./server
ENV PORT=50201 DLV_PORT=40000
EXPOSE 50201 40000
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40000", "--api-version=2", "--accept-multiclient", "./server"]

FROM runtime-debug AS agg-table-debug
COPY --from=build-table-debug /out/server ./server
ENV PORT=50202 DLV_PORT=40001
EXPOSE 50202 40001
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40001", "--api-version=2", "--accept-multiclient", "./server"]

FROM runtime-debug AS agg-hand-debug
COPY --from=build-hand-debug /out/server ./server
ENV PORT=50203 DLV_PORT=40002
EXPOSE 50203 40002
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40002", "--api-version=2", "--accept-multiclient", "./server"]

# ============================================================================
# Debug Sagas
# ============================================================================
FROM runtime-debug AS saga-table-hand-debug
COPY --from=build-saga-table-hand-debug /out/server ./server
ENV PORT=50211 DLV_PORT=40003
EXPOSE 50211 40003
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40003", "--api-version=2", "--accept-multiclient", "./server"]

FROM runtime-debug AS saga-table-player-debug
COPY --from=build-saga-table-player-debug /out/server ./server
ENV PORT=50212 DLV_PORT=40004
EXPOSE 50212 40004
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40004", "--api-version=2", "--accept-multiclient", "./server"]

FROM runtime-debug AS saga-hand-table-debug
COPY --from=build-saga-hand-table-debug /out/server ./server
ENV PORT=50213 DLV_PORT=40005
EXPOSE 50213 40005
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40005", "--api-version=2", "--accept-multiclient", "./server"]

FROM runtime-debug AS saga-hand-player-debug
COPY --from=build-saga-hand-player-debug /out/server ./server
ENV PORT=50214 DLV_PORT=40006
EXPOSE 50214 40006
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40006", "--api-version=2", "--accept-multiclient", "./server"]

# ============================================================================
# Debug Process Manager
# ============================================================================
FROM runtime-debug AS pmg-hand-flow-debug
COPY --from=build-pmg-hand-flow-debug /out/server ./server
ENV PORT=50291 DLV_PORT=40007
EXPOSE 50291 40007
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40007", "--api-version=2", "--accept-multiclient", "./server"]

# ============================================================================
# Debug Projector
# ============================================================================
FROM runtime-debug AS prj-output-debug
COPY --from=build-prj-output-debug /out/server ./server
ENV PORT=50290 DLV_PORT=40008
EXPOSE 50290 40008
ENTRYPOINT ["dlv", "exec", "--headless", "--listen=:40008", "--api-version=2", "--accept-multiclient", "./server"]
