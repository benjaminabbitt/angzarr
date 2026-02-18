#pragma once

#include <stdexcept>
#include <string>
#include <grpcpp/grpcpp.h>

namespace angzarr {

/// Exception thrown when a command is rejected by the aggregate.
class CommandRejectedError : public std::runtime_error {
public:
    grpc::StatusCode status_code;

    CommandRejectedError(const std::string& message, grpc::StatusCode code = grpc::StatusCode::UNKNOWN)
        : std::runtime_error(message), status_code(code) {}

    static CommandRejectedError precondition_failed(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::FAILED_PRECONDITION);
    }

    static CommandRejectedError invalid_argument(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::INVALID_ARGUMENT);
    }

    static CommandRejectedError not_found(const std::string& message) {
        return CommandRejectedError(message, grpc::StatusCode::NOT_FOUND);
    }
};

} // namespace angzarr
