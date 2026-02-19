#include <gtest/gtest.h>
#include <string>
#include <google/protobuf/any.pb.h>
#include "angzarr/types.pb.h"
#include "angzarr/aggregate.pb.h"
#include "angzarr/query.pb.h"
#include "angzarr/builder.hpp"

using namespace angzarr;

// =============================================================================
// CommandBuilder Tests
// =============================================================================

class CommandBuilderTest : public ::testing::Test {
protected:
    // Test UUID bytes (550e8400-e29b-41d4-a716-446655440000)
    std::string test_root_bytes() {
        // UUID in little-endian format as used by the builder
        return std::string("\x00\xe4\x50\x55\x9b\xe2\xd4\x41\xa7\x16\x44\x66\x55\x44\x00\x00", 16);
    }
};

TEST_F(CommandBuilderTest, Build_WithExplicitFieldValues_ShouldSetAllFields) {
    // When I build a command using CommandBuilder with explicit values
    auto root = test_root_bytes();
    std::string correlation_id = "corr-123";
    uint32_t sequence = 5;

    // Create a simple test message
    google::protobuf::Any test_msg;
    test_msg.set_type_url("type.googleapis.com/test.TestCommand");
    test_msg.set_value("test payload");

    CommandBuilder builder(nullptr, "test");
    builder.with_root(root)
           .with_correlation_id(correlation_id)
           .with_sequence(sequence)
           .with_command("type.googleapis.com/test.TestCommand", test_msg);

    auto command = builder.build();

    // Then the resulting CommandBook should have the specified values
    EXPECT_EQ(command.cover().domain(), "test");
    EXPECT_EQ(command.cover().correlation_id(), correlation_id);
    EXPECT_EQ(command.pages(0).sequence(), sequence);
    EXPECT_EQ(command.pages(0).command().type_url(), "type.googleapis.com/test.TestCommand");
}

TEST_F(CommandBuilderTest, Build_WithoutCorrelationId_ShouldAutoGenerateOne) {
    // When I build a command without specifying correlation_id
    google::protobuf::Any test_msg;
    test_msg.set_type_url("type.googleapis.com/test.TestCommand");

    CommandBuilder builder(nullptr, "test");
    builder.with_command("type.googleapis.com/test.TestCommand", test_msg);

    auto command = builder.build();

    // Then the resulting CommandBook should have a non-empty correlation_id
    EXPECT_FALSE(command.cover().correlation_id().empty());
    // Should be in UUID format (36 chars with dashes)
    EXPECT_EQ(command.cover().correlation_id().length(), 36);
}

TEST_F(CommandBuilderTest, Build_ForNewAggregate_ShouldHaveNoRootUUID) {
    // When I build a command for domain "test" without specifying root
    google::protobuf::Any test_msg;
    test_msg.set_type_url("type.googleapis.com/test.TestCommand");

    CommandBuilder builder(nullptr, "test");
    builder.with_command("type.googleapis.com/test.TestCommand", test_msg);

    auto command = builder.build();

    // Then the resulting CommandBook should have no root UUID
    EXPECT_FALSE(command.cover().has_root());
}

TEST_F(CommandBuilderTest, Build_WithoutSequence_ShouldDefaultToZero) {
    // When I build a command without specifying sequence
    google::protobuf::Any test_msg;
    test_msg.set_type_url("type.googleapis.com/test.TestCommand");

    CommandBuilder builder(nullptr, "test");
    builder.with_command("type.googleapis.com/test.TestCommand", test_msg);

    auto command = builder.build();

    // Then the resulting CommandBook should have sequence 0
    EXPECT_EQ(command.pages(0).sequence(), 0u);
}

TEST_F(CommandBuilderTest, MethodChaining_ShouldReturnBuilder) {
    // Verify method chaining returns builder for fluent composition
    google::protobuf::Any test_msg;
    test_msg.set_type_url("type.googleapis.com/test.TestCommand");

    CommandBuilder builder(nullptr, "test");

    auto& result1 = builder.with_correlation_id("chain-test");
    auto& result2 = result1.with_sequence(10);
    auto& result3 = result2.with_command("type.googleapis.com/test.TestCommand", test_msg);

    // All results should be the same builder
    EXPECT_EQ(&result1, &builder);
    EXPECT_EQ(&result2, &builder);
    EXPECT_EQ(&result3, &builder);

    auto command = builder.build();
    EXPECT_EQ(command.cover().correlation_id(), "chain-test");
    EXPECT_EQ(command.pages(0).sequence(), 10u);
}

TEST_F(CommandBuilderTest, Build_WithoutCommand_ShouldThrow) {
    // When trying to build without setting a command
    CommandBuilder builder(nullptr, "test");

    // Then it should throw InvalidArgumentError
    EXPECT_THROW(builder.build(), InvalidArgumentError);
}

// =============================================================================
// QueryBuilder Tests
// =============================================================================

class QueryBuilderTest : public ::testing::Test {
protected:
    std::string test_root_bytes() {
        return std::string("\x00\xe4\x50\x55\x9b\xe2\xd4\x41\xa7\x16\x44\x66\x55\x44\x00\x00", 16);
    }
};

TEST_F(QueryBuilderTest, Build_WithDomainAndRoot_ShouldSetBothFields) {
    // When I build a query with domain and root
    auto root = test_root_bytes();

    QueryBuilder builder(nullptr, "test");
    builder.with_root(root);

    auto query = builder.build();

    // Then the resulting Query should have both fields set
    EXPECT_EQ(query.cover().domain(), "test");
    EXPECT_TRUE(query.cover().has_root());
}

TEST_F(QueryBuilderTest, Build_WithRangeTo_ShouldSetBothBounds) {
    // When I build a query with range from 5 to 10
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes())
           .range_to(5, 10);

    auto query = builder.build();

    // Then the resulting Query should have sequence_range with lower=5 and upper=10
    EXPECT_TRUE(query.has_range());
    EXPECT_EQ(query.range().lower(), 5u);
    EXPECT_EQ(query.range().upper(), 10u);
}

TEST_F(QueryBuilderTest, Build_WithRangeOpenEnded_ShouldOnlySetLowerBound) {
    // When I build a query with range from 5
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes())
           .range(5);

    auto query = builder.build();

    // Then the resulting Query should have sequence_range with lower=5 and no upper bound
    EXPECT_TRUE(query.has_range());
    EXPECT_EQ(query.range().lower(), 5u);
    EXPECT_EQ(query.range().upper(), 0u); // Not set, default value
}

TEST_F(QueryBuilderTest, Build_AsOfSequence_ShouldSetTemporalSequence) {
    // When I build a query as_of_sequence 42
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes())
           .as_of_sequence(42);

    auto query = builder.build();

    // Then the resulting Query should have temporal_query with sequence=42
    EXPECT_TRUE(query.has_temporal());
    EXPECT_EQ(query.temporal().as_of_sequence(), 42u);
}

TEST_F(QueryBuilderTest, Build_AsOfTime_ShouldParseTimestamp) {
    // When I build a query as_of_time "2024-01-15T10:30:00Z"
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes())
           .as_of_time("2024-01-15T10:30:00Z");

    auto query = builder.build();

    // Then the resulting Query should have temporal_query with the parsed timestamp
    EXPECT_TRUE(query.has_temporal());
    EXPECT_TRUE(query.temporal().has_as_of_time());
    // January 15, 2024 10:30:00 UTC = 1705314600 seconds since Unix epoch
    EXPECT_EQ(query.temporal().as_of_time().seconds(), 1705314600);
}

TEST_F(QueryBuilderTest, Build_ByCorrelationId_ShouldClearRoot) {
    // When I build a query by_correlation_id "corr-456"
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes())  // Set root first
           .by_correlation_id("corr-456"); // This should clear root

    auto query = builder.build();

    // Then the resulting Query should query by correlation_id
    EXPECT_EQ(query.cover().correlation_id(), "corr-456");
    EXPECT_FALSE(query.cover().has_root());
}

TEST_F(QueryBuilderTest, Build_WithEdition_ShouldSetEditionName) {
    // When I build a query with_edition "v2"
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes())
           .with_edition("v2");

    auto query = builder.build();

    // Then the resulting Query should have edition "v2"
    EXPECT_TRUE(query.cover().has_edition());
    EXPECT_EQ(query.cover().edition().name(), "v2");
}

TEST_F(QueryBuilderTest, Build_InvalidTimestamp_ShouldThrow) {
    // When I build a query with an invalid timestamp
    QueryBuilder builder(nullptr, "test");
    builder.with_root(test_root_bytes());

    // Then as_of_time with invalid timestamp should throw
    EXPECT_THROW(builder.as_of_time("not-a-timestamp"), InvalidTimestampError);
}
