# syntax=docker/dockerfile:1.4
# C# poker examples - optimized multi-stage build
# Build: podman build -t poker-csharp-player --target agg-player -f examples/csharp/Containerfile .
# Context must be repo root for client library access
#
# Optimizations:
# 1. Shared restore stage - NuGet restore runs once
# 2. Named cache IDs for NuGet package cache persistence
# 3. Slim Debian runtime - minimal attack surface
# 4. Multi-arch support (amd64 + arm64)
#
# Note: Using Debian-based images (not Alpine) because Grpc.Tools NuGet package
# bundles glibc-linked protoc binaries that don't run on musl-based Alpine.

ARG DOTNET_VERSION=8.0

# ============================================================================
# Base builder - .NET SDK (Debian bookworm)
# ============================================================================
FROM mcr.microsoft.com/dotnet/sdk:${DOTNET_VERSION}-bookworm-slim AS base

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ============================================================================
# Restore - download all NuGet dependencies once
# ============================================================================
FROM base AS restore

WORKDIR /app

# Copy proto files (required by Angzarr.Proto for proto compilation)
COPY proto ./proto

# Copy client library (local dependency)
COPY client/csharp ./client/csharp

# Copy solution and project files for dependency resolution
COPY examples/csharp/Angzarr.Examples.sln ./examples/csharp/
COPY examples/csharp/Angzarr.Proto/Angzarr.Proto.csproj ./examples/csharp/Angzarr.Proto/
COPY examples/csharp/Player/Agg/Player.Agg.csproj ./examples/csharp/Player/Agg/
COPY examples/csharp/Table/Agg/Table.Agg.csproj ./examples/csharp/Table/Agg/
COPY examples/csharp/Hand/Agg/Hand.Agg.csproj ./examples/csharp/Hand/Agg/
COPY examples/csharp/Table/SagaHand/Table.SagaHand.csproj ./examples/csharp/Table/SagaHand/
COPY examples/csharp/Table/SagaPlayer/Table.SagaPlayer.csproj ./examples/csharp/Table/SagaPlayer/
COPY examples/csharp/Hand/SagaTable/Hand.SagaTable.csproj ./examples/csharp/Hand/SagaTable/
COPY examples/csharp/Hand/SagaPlayer/Hand.SagaPlayer.csproj ./examples/csharp/Hand/SagaPlayer/
COPY examples/csharp/HandFlow/HandFlow.csproj ./examples/csharp/HandFlow/
COPY examples/csharp/PrjOutput/PrjOutput.csproj ./examples/csharp/PrjOutput/
COPY examples/csharp/Tests/Tests.csproj ./examples/csharp/Tests/
COPY examples/csharp/HandFlowOO/HandFlowOO.csproj ./examples/csharp/HandFlowOO/
COPY examples/csharp/Player/Upc/Player.Upc.csproj ./examples/csharp/Player/Upc/
COPY examples/csharp/PrjCloudEvents/PrjCloudEvents.csproj ./examples/csharp/PrjCloudEvents/
COPY examples/csharp/Table/SagaHandOO/Table.SagaHandOO.csproj ./examples/csharp/Table/SagaHandOO/
COPY examples/csharp/PrjOutputOO/PrjOutputOO.csproj ./examples/csharp/PrjOutputOO/
COPY examples/csharp/Player/SagaTable/Player.SagaTable.csproj ./examples/csharp/Player/SagaTable/

WORKDIR /app/examples/csharp

# Restore with persistent cache
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet restore

# ============================================================================
# Source - copy all C# source
# ============================================================================
FROM restore AS source

# Copy all source files
COPY examples/csharp/ ./

# ============================================================================
# Aggregate builds
# ============================================================================
FROM source AS build-player
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Player/Agg/Player.Agg.csproj -c Release -o /out --no-restore

FROM source AS build-table
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Table/Agg/Table.Agg.csproj -c Release -o /out --no-restore

FROM source AS build-hand
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Hand/Agg/Hand.Agg.csproj -c Release -o /out --no-restore

# ============================================================================
# Saga builds
# ============================================================================
FROM source AS build-saga-table-hand
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Table/SagaHand/Table.SagaHand.csproj -c Release -o /out --no-restore

FROM source AS build-saga-table-player
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Table/SagaPlayer/Table.SagaPlayer.csproj -c Release -o /out --no-restore

FROM source AS build-saga-hand-table
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Hand/SagaTable/Hand.SagaTable.csproj -c Release -o /out --no-restore

FROM source AS build-saga-hand-player
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish Hand/SagaPlayer/Hand.SagaPlayer.csproj -c Release -o /out --no-restore

# ============================================================================
# Process Manager build
# ============================================================================
FROM source AS build-pmg-hand-flow
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish HandFlow/HandFlow.csproj -c Release -o /out --no-restore

# ============================================================================
# Projector build
# ============================================================================
FROM source AS build-prj-output
WORKDIR /app/examples/csharp
RUN --mount=type=cache,id=nuget-cache,target=/root/.nuget/packages,sharing=locked \
    dotnet publish PrjOutput/PrjOutput.csproj -c Release -o /out --no-restore

# ============================================================================
# Runtime base - ASP.NET Core runtime (required for gRPC)
# ============================================================================
FROM mcr.microsoft.com/dotnet/aspnet:${DOTNET_VERSION}-bookworm-slim AS runtime
WORKDIR /app
RUN adduser --disabled-password --gecos "" --uid 1000 angzarr
USER angzarr

# ============================================================================
# Domain Aggregates
# ============================================================================
FROM runtime AS agg-player
COPY --from=build-player --chown=angzarr:angzarr /out ./
ENV PORT=50501 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50501
ENTRYPOINT ["./Player.Agg"]

FROM runtime AS agg-table
COPY --from=build-table --chown=angzarr:angzarr /out ./
ENV PORT=50502 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50502
ENTRYPOINT ["./Table.Agg"]

FROM runtime AS agg-hand
COPY --from=build-hand --chown=angzarr:angzarr /out ./
ENV PORT=50503 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50503
ENTRYPOINT ["./Hand.Agg"]

# ============================================================================
# Sagas
# ============================================================================
FROM runtime AS saga-table-hand
COPY --from=build-saga-table-hand --chown=angzarr:angzarr /out ./
ENV PORT=50511 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50511
ENTRYPOINT ["./Table.SagaHand"]

FROM runtime AS saga-table-player
COPY --from=build-saga-table-player --chown=angzarr:angzarr /out ./
ENV PORT=50512 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50512
ENTRYPOINT ["./Table.SagaPlayer"]

FROM runtime AS saga-hand-table
COPY --from=build-saga-hand-table --chown=angzarr:angzarr /out ./
ENV PORT=50513 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50513
ENTRYPOINT ["./Hand.SagaTable"]

FROM runtime AS saga-hand-player
COPY --from=build-saga-hand-player --chown=angzarr:angzarr /out ./
ENV PORT=50514 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50514
ENTRYPOINT ["./Hand.SagaPlayer"]

# ============================================================================
# Process Manager
# ============================================================================
FROM runtime AS pmg-hand-flow
COPY --from=build-pmg-hand-flow --chown=angzarr:angzarr /out ./
ENV PORT=50591 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50591
ENTRYPOINT ["./HandFlow"]

# ============================================================================
# Projector
# ============================================================================
FROM runtime AS prj-output
COPY --from=build-prj-output --chown=angzarr:angzarr /out ./
ENV PORT=50590 \
    DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1
EXPOSE 50590
ENTRYPOINT ["./PrjOutput"]
