# syntax=docker/dockerfile:1.4
# Java poker examples - optimized multi-stage build
# Build: podman build -t poker-java-player --target agg-player -f examples/java/Containerfile .
# Context must be repo root for client library access
#
# Optimizations:
# 1. Shared gradle dependency download - runs once
# 2. Named cache IDs for Gradle cache persistence
# 3. Distroless Java runtime - minimal attack surface
# 4. Multi-arch support (amd64 + arm64)
#
# Note: Using Ubuntu-based Temurin (not Alpine) because io.grpc:protoc-gen-grpc-java
# bundles glibc-linked binaries that don't run on musl-based Alpine.

ARG JAVA_VERSION=21

# ============================================================================
# Base builder - Gradle with JDK (Ubuntu jammy)
# ============================================================================
FROM docker.io/library/eclipse-temurin:${JAVA_VERSION}-jdk-jammy AS base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates bash \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# ============================================================================
# Dependencies - download all Gradle deps once
# ============================================================================
FROM base AS deps

WORKDIR /app

# Copy proto files (required by client/java/proto for proto compilation)
COPY proto ./proto

# Copy client library (composite build dependency)
COPY client/java ./client/java

# Copy gradle wrapper and build files
COPY examples/java/gradle ./examples/java/gradle
COPY examples/java/gradlew ./examples/java/
COPY examples/java/build.gradle.kts ./examples/java/
COPY examples/java/settings.gradle.kts ./examples/java/

# Copy all project build files for dependency resolution
COPY examples/java/player/agg/build.gradle.kts ./examples/java/player/agg/
COPY examples/java/table/agg/build.gradle.kts ./examples/java/table/agg/
COPY examples/java/hand/agg/build.gradle.kts ./examples/java/hand/agg/
COPY examples/java/table/saga-hand/build.gradle.kts ./examples/java/table/saga-hand/
COPY examples/java/table/saga-player/build.gradle.kts ./examples/java/table/saga-player/
COPY examples/java/hand/saga-table/build.gradle.kts ./examples/java/hand/saga-table/
COPY examples/java/hand/saga-player/build.gradle.kts ./examples/java/hand/saga-player/
COPY examples/java/hand-flow/build.gradle.kts ./examples/java/hand-flow/
COPY examples/java/prj-output/build.gradle.kts ./examples/java/prj-output/
COPY examples/java/tests/build.gradle.kts ./examples/java/tests/

# Create source stubs for dependency resolution
RUN mkdir -p examples/java/player/agg/src/main/java \
    examples/java/table/agg/src/main/java \
    examples/java/hand/agg/src/main/java \
    examples/java/table/saga-hand/src/main/java \
    examples/java/table/saga-player/src/main/java \
    examples/java/hand/saga-table/src/main/java \
    examples/java/hand/saga-player/src/main/java \
    examples/java/hand-flow/src/main/java \
    examples/java/prj-output/src/main/java \
    examples/java/tests/src/test/java

WORKDIR /app/examples/java

# Download dependencies with persistent cache
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    chmod +x ./gradlew && ./gradlew dependencies --no-daemon

# ============================================================================
# Source - copy all Java source
# ============================================================================
FROM deps AS source

WORKDIR /app

# Copy all source files
COPY examples/java/player ./examples/java/player
COPY examples/java/table ./examples/java/table
COPY examples/java/hand ./examples/java/hand
COPY examples/java/hand-flow ./examples/java/hand-flow
COPY examples/java/prj-output ./examples/java/prj-output

# ============================================================================
# Aggregate builds (Spring Boot uses bootJar, not shadowJar)
# ============================================================================
FROM source AS build-player
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :player-agg:bootJar --no-daemon \
    && cp player/agg/build/libs/*.jar /out.jar

FROM source AS build-table
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :table-agg:bootJar --no-daemon \
    && cp table/agg/build/libs/*.jar /out.jar

FROM source AS build-hand
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :hand-agg:bootJar --no-daemon \
    && cp hand/agg/build/libs/*.jar /out.jar

# ============================================================================
# Saga builds
# ============================================================================
FROM source AS build-saga-table-hand
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :table-saga-hand:bootJar --no-daemon \
    && cp table/saga-hand/build/libs/*.jar /out.jar

FROM source AS build-saga-table-player
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :table-saga-player:bootJar --no-daemon \
    && cp table/saga-player/build/libs/*.jar /out.jar

FROM source AS build-saga-hand-table
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :hand-saga-table:bootJar --no-daemon \
    && cp hand/saga-table/build/libs/*.jar /out.jar

FROM source AS build-saga-hand-player
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :hand-saga-player:bootJar --no-daemon \
    && cp hand/saga-player/build/libs/*.jar /out.jar

# ============================================================================
# Process Manager build
# ============================================================================
FROM source AS build-pmg-hand-flow
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :hand-flow:bootJar --no-daemon \
    && cp hand-flow/build/libs/*.jar /out.jar

# ============================================================================
# Projector build
# ============================================================================
FROM source AS build-prj-output
WORKDIR /app/examples/java
RUN --mount=type=cache,id=gradle-cache,target=/root/.gradle,sharing=locked \
    ./gradlew :prj-output:bootJar --no-daemon \
    && cp prj-output/build/libs/*.jar /out.jar

# ============================================================================
# Runtime base - distroless Java (minimal, secure)
# ============================================================================
FROM gcr.io/distroless/java${JAVA_VERSION}-debian12:nonroot AS runtime
WORKDIR /app
USER nonroot:nonroot

# ============================================================================
# Domain Aggregates
# ============================================================================
FROM runtime AS agg-player
COPY --from=build-player --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50601
EXPOSE 50601
ENTRYPOINT ["java", "-jar", "app.jar"]

FROM runtime AS agg-table
COPY --from=build-table --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50602
EXPOSE 50602
ENTRYPOINT ["java", "-jar", "app.jar"]

FROM runtime AS agg-hand
COPY --from=build-hand --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50603
EXPOSE 50603
ENTRYPOINT ["java", "-jar", "app.jar"]

# ============================================================================
# Sagas
# ============================================================================
FROM runtime AS saga-table-hand
COPY --from=build-saga-table-hand --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50611
EXPOSE 50611
ENTRYPOINT ["java", "-jar", "app.jar"]

FROM runtime AS saga-table-player
COPY --from=build-saga-table-player --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50612
EXPOSE 50612
ENTRYPOINT ["java", "-jar", "app.jar"]

FROM runtime AS saga-hand-table
COPY --from=build-saga-hand-table --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50613
EXPOSE 50613
ENTRYPOINT ["java", "-jar", "app.jar"]

FROM runtime AS saga-hand-player
COPY --from=build-saga-hand-player --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50614
EXPOSE 50614
ENTRYPOINT ["java", "-jar", "app.jar"]

# ============================================================================
# Process Manager
# ============================================================================
FROM runtime AS pmg-hand-flow
COPY --from=build-pmg-hand-flow --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50691
EXPOSE 50691
ENTRYPOINT ["java", "-jar", "app.jar"]

# ============================================================================
# Projector
# ============================================================================
FROM runtime AS prj-output
COPY --from=build-prj-output --chown=nonroot:nonroot /out.jar ./app.jar
ENV PORT=50690
EXPOSE 50690
ENTRYPOINT ["java", "-jar", "app.jar"]
