#pragma once

#include <stdexcept>
#include <string>
#include <grpcpp/grpcpp.h>

namespace angzarr {

/**
 * Base exception for all Angzarr client errors.
 */
class ClientError : public std::runtime_error {
public:
    explicit ClientError(const std::string& message)
        : std::runtime_error(message) {}
};

/**
 * Thrown when a command is rejected by business logic.
 * Maps to gRPC FAILED_PRECONDITION status.
 */
class CommandRejectedError : public ClientError {
public:
    explicit CommandRejectedError(const std::string& message)
        : ClientError(message) {}
};

/**
 * Thrown when a gRPC call fails.
 */
class GrpcError : public ClientError {
public:
    GrpcError(const std::string& message, grpc::StatusCode status_code)
        : ClientError(message), status_code_(status_code) {}

    grpc::StatusCode status_code() const { return status_code_; }

private:
    grpc::StatusCode status_code_;
};

/**
 * Thrown when connection to the server fails.
 */
class ConnectionError : public ClientError {
public:
    explicit ConnectionError(const std::string& message)
        : ClientError(message) {}
};

/**
 * Thrown when transport-level errors occur.
 */
class TransportError : public ClientError {
public:
    explicit TransportError(const std::string& message)
        : ClientError(message) {}
};

/**
 * Thrown when an invalid argument is provided.
 */
class InvalidArgumentError : public ClientError {
public:
    explicit InvalidArgumentError(const std::string& message)
        : ClientError(message) {}
};

/**
 * Thrown when a timestamp cannot be parsed.
 */
class InvalidTimestampError : public ClientError {
public:
    explicit InvalidTimestampError(const std::string& message)
        : ClientError(message) {}
};

} // namespace angzarr
