#pragma once

#include <grpcpp/grpcpp.h>

#include <stdexcept>
#include <string>

namespace angzarr {

/**
 * Base exception for all Angzarr client errors.
 */
class ClientError : public std::runtime_error {
   public:
    explicit ClientError(const std::string& message) : std::runtime_error(message) {}

    /**
     * Returns true if this is a "not found" error.
     */
    virtual bool is_not_found() const { return false; }

    /**
     * Returns true if this is a "precondition failed" error.
     */
    virtual bool is_precondition_failed() const { return false; }

    /**
     * Returns true if this is an "invalid argument" error.
     */
    virtual bool is_invalid_argument() const { return false; }

    /**
     * Returns true if this is a connection or transport error.
     */
    virtual bool is_connection_error() const { return false; }
};

/**
 * Thrown when a command is rejected by business logic.
 * Maps to gRPC FAILED_PRECONDITION status.
 */
class CommandRejectedError : public ClientError {
   public:
    explicit CommandRejectedError(const std::string& message)
        : ClientError(message), status_code(grpc::StatusCode::FAILED_PRECONDITION) {}

    CommandRejectedError(const std::string& message, grpc::StatusCode code)
        : ClientError(message), status_code(code) {}

    bool is_precondition_failed() const override { return true; }

    /// gRPC status code for this rejection.
    grpc::StatusCode status_code;

    /// Factory methods for common rejection types.
    static CommandRejectedError precondition_failed(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::FAILED_PRECONDITION);
    }

    static CommandRejectedError invalid_argument(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::INVALID_ARGUMENT);
    }

    static CommandRejectedError not_found(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::NOT_FOUND);
    }

    static CommandRejectedError already_exists(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::ALREADY_EXISTS);
    }
};

/**
 * Thrown when a gRPC call fails.
 */
class GrpcError : public ClientError {
   public:
    GrpcError(const std::string& message, grpc::StatusCode status_code)
        : ClientError(message), status_code_(status_code) {}

    grpc::StatusCode status_code() const { return status_code_; }

    bool is_not_found() const override { return status_code_ == grpc::StatusCode::NOT_FOUND; }

    bool is_precondition_failed() const override {
        return status_code_ == grpc::StatusCode::FAILED_PRECONDITION;
    }

    bool is_invalid_argument() const override {
        return status_code_ == grpc::StatusCode::INVALID_ARGUMENT;
    }

    bool is_connection_error() const override {
        return status_code_ == grpc::StatusCode::UNAVAILABLE;
    }

   private:
    grpc::StatusCode status_code_;
};

/**
 * Thrown when connection to the server fails.
 */
class ConnectionError : public ClientError {
   public:
    explicit ConnectionError(const std::string& message) : ClientError(message) {}

    bool is_connection_error() const override { return true; }
};

/**
 * Thrown when transport-level errors occur.
 */
class TransportError : public ClientError {
   public:
    explicit TransportError(const std::string& message) : ClientError(message) {}

    bool is_connection_error() const override { return true; }
};

/**
 * Thrown when an invalid argument is provided.
 */
class InvalidArgumentError : public ClientError {
   public:
    explicit InvalidArgumentError(const std::string& message) : ClientError(message) {}

    bool is_invalid_argument() const override { return true; }
};

/**
 * Thrown when a timestamp cannot be parsed.
 */
class InvalidTimestampError : public ClientError {
   public:
    explicit InvalidTimestampError(const std::string& message) : ClientError(message) {}
};

}  // namespace angzarr
