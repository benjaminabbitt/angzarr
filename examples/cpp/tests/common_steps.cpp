// GTest must be included before cucumber-cpp autodetect for framework detection
#include <gtest/gtest.h>
#include <cucumber-cpp/autodetect.hpp>

#include "test_context.hpp"

using cucumber::ScenarioScope;

/// Before each scenario, reset the context.
BEFORE() { tests::g_context.reset(); }

/// Common step: verify command failed with specific status.
THEN("^the command fails with status \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected_status);
    ScenarioScope<tests::ScenarioContext> ctx;

    ASSERT_TRUE(tests::g_context.has_error())
        << "Expected command to fail, but it succeeded";

    auto expected_code = tests::string_to_status_code(expected_status);
    ASSERT_TRUE(tests::g_context.last_error_code.has_value())
        << "Expected error code to be set";

    ASSERT_EQ(tests::g_context.last_error_code.value(), expected_code)
        << "Expected status " << expected_status << " but got "
        << tests::status_code_to_string(tests::g_context.last_error_code.value());
}

/// Common step: verify error message contains substring (case-insensitive).
THEN("^the error message contains \"([^\"]*)\"$") {
    REGEX_PARAM(std::string, expected_substring);

    ASSERT_TRUE(tests::g_context.has_error()) << "Expected command to have failed";
    ASSERT_TRUE(tests::g_context.last_error.has_value()) << "Expected error message to be set";

    // Convert both to lowercase for case-insensitive comparison
    std::string error_msg = tests::g_context.last_error.value();
    std::string expected_lower = expected_substring;
    std::transform(error_msg.begin(), error_msg.end(), error_msg.begin(), ::tolower);
    std::transform(expected_lower.begin(), expected_lower.end(), expected_lower.begin(), ::tolower);

    ASSERT_TRUE(error_msg.find(expected_lower) != std::string::npos)
        << "Expected error message to contain '" << expected_substring << "' but got: '"
        << tests::g_context.last_error.value() << "'";
}
