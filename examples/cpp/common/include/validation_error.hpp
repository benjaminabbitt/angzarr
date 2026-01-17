#pragma once

#include <grpcpp/grpcpp.h>
#include <stdexcept>
#include <string>

namespace angzarr {

class ValidationError : public std::runtime_error {
public:
    enum class StatusCode {
        InvalidArgument,
        FailedPrecondition
    };

    ValidationError(const std::string& message, StatusCode code)
        : std::runtime_error(message), code_(code) {}

    static ValidationError invalid_argument(const std::string& message) {
        return ValidationError(message, StatusCode::InvalidArgument);
    }

    static ValidationError failed_precondition(const std::string& message) {
        return ValidationError(message, StatusCode::FailedPrecondition);
    }

    grpc::Status to_grpc_status() const {
        switch (code_) {
            case StatusCode::InvalidArgument:
                return grpc::Status(grpc::StatusCode::INVALID_ARGUMENT, what());
            case StatusCode::FailedPrecondition:
                return grpc::Status(grpc::StatusCode::FAILED_PRECONDITION, what());
            default:
                return grpc::Status(grpc::StatusCode::UNKNOWN, what());
        }
    }

private:
    StatusCode code_;
};

}  // namespace angzarr
