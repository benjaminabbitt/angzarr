#include <gtest/gtest.h>
#include "angzarr/errors.hpp"

using namespace angzarr;

// =============================================================================
// IsNotFound Tests
// =============================================================================

TEST(ErrorIntrospectionTest, GrpcError_WithNotFound_ShouldReturnTrueForIsNotFound) {
    GrpcError error("not found", grpc::StatusCode::NOT_FOUND);
    EXPECT_TRUE(error.is_not_found());
}

TEST(ErrorIntrospectionTest, GrpcError_WithOtherCode_ShouldReturnFalseForIsNotFound) {
    GrpcError error("internal error", grpc::StatusCode::INTERNAL);
    EXPECT_FALSE(error.is_not_found());
}

// =============================================================================
// IsPreconditionFailed Tests
// =============================================================================

TEST(ErrorIntrospectionTest, GrpcError_WithFailedPrecondition_ShouldReturnTrueForIsPreconditionFailed) {
    GrpcError error("precondition failed", grpc::StatusCode::FAILED_PRECONDITION);
    EXPECT_TRUE(error.is_precondition_failed());
}

TEST(ErrorIntrospectionTest, GrpcError_WithOtherCode_ShouldReturnFalseForIsPreconditionFailed) {
    GrpcError error("internal error", grpc::StatusCode::INTERNAL);
    EXPECT_FALSE(error.is_precondition_failed());
}

TEST(ErrorIntrospectionTest, CommandRejectedError_ShouldReturnTrueForIsPreconditionFailed) {
    CommandRejectedError error("rejected");
    EXPECT_TRUE(error.is_precondition_failed());
}

// =============================================================================
// IsInvalidArgument Tests
// =============================================================================

TEST(ErrorIntrospectionTest, GrpcError_WithInvalidArgument_ShouldReturnTrueForIsInvalidArgument) {
    GrpcError error("invalid argument", grpc::StatusCode::INVALID_ARGUMENT);
    EXPECT_TRUE(error.is_invalid_argument());
}

TEST(ErrorIntrospectionTest, InvalidArgumentError_ShouldReturnTrueForIsInvalidArgument) {
    InvalidArgumentError error("bad input");
    EXPECT_TRUE(error.is_invalid_argument());
}

TEST(ErrorIntrospectionTest, GrpcError_WithOtherCode_ShouldReturnFalseForIsInvalidArgument) {
    GrpcError error("internal error", grpc::StatusCode::INTERNAL);
    EXPECT_FALSE(error.is_invalid_argument());
}

// =============================================================================
// IsConnectionError Tests
// =============================================================================

TEST(ErrorIntrospectionTest, ConnectionError_ShouldReturnTrueForIsConnectionError) {
    ConnectionError error("connection refused");
    EXPECT_TRUE(error.is_connection_error());
}

TEST(ErrorIntrospectionTest, TransportError_ShouldReturnTrueForIsConnectionError) {
    TransportError error("transport failed");
    EXPECT_TRUE(error.is_connection_error());
}

TEST(ErrorIntrospectionTest, GrpcError_WithUnavailable_ShouldReturnTrueForIsConnectionError) {
    GrpcError error("unavailable", grpc::StatusCode::UNAVAILABLE);
    EXPECT_TRUE(error.is_connection_error());
}

TEST(ErrorIntrospectionTest, GrpcError_WithOtherCode_ShouldReturnFalseForIsConnectionError) {
    GrpcError error("internal error", grpc::StatusCode::INTERNAL);
    EXPECT_FALSE(error.is_connection_error());
}

// =============================================================================
// Base Class Default Behavior Tests
// =============================================================================

TEST(ErrorIntrospectionTest, ClientError_ShouldHaveDefaultFalseForAllIntrospectionMethods) {
    ClientError error("generic error");
    EXPECT_FALSE(error.is_not_found());
    EXPECT_FALSE(error.is_precondition_failed());
    EXPECT_FALSE(error.is_invalid_argument());
    EXPECT_FALSE(error.is_connection_error());
}
