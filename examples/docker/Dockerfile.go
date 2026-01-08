# Go gRPC service - multi-stage build
FROM golang:1.23-alpine AS builder

# Install build dependencies
RUN apk add --no-cache git

# Set working directory
WORKDIR /app

# Copy go mod files
COPY examples/go/${SERVICE}/go.mod examples/go/${SERVICE}/go.sum ./

# Copy proto files
COPY generated/go/evented proto/evented/
COPY generated/go/examples proto/examples/

# Download dependencies
RUN go mod download

# Copy source code
COPY examples/go/${SERVICE}/*.go ./

# Build the binary
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o server .

# Final stage - minimal image
FROM alpine:3.19

# Install ca-certificates for TLS
RUN apk --no-cache add ca-certificates

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/server .

# Run the server
CMD ["./server"]
