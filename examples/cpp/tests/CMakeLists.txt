# Cucumber-cpp BDD acceptance tests for C++ poker examples
#
# This module sets up cucumber-cpp with GTest backend to run
# acceptance tests defined in Gherkin feature files.

include(FetchContent)

# Find GTest (system-installed, required for cucumber-cpp)
find_package(GTest 1.11.0 REQUIRED)

# Find Boost (required by cucumber-cpp for regex)
cmake_policy(SET CMP0167 NEW)
find_package(Boost 1.70 CONFIG QUIET COMPONENTS regex filesystem system program_options thread)
if(NOT Boost_FOUND)
    find_package(Boost 1.70 REQUIRED COMPONENTS regex filesystem system program_options thread)
endif()

# Find nlohmann_json (required by cucumber-cpp)
find_package(nlohmann_json 3.10 REQUIRED)

# Set cucumber-cpp options before FetchContent
set(CUKE_ENABLE_QT OFF CACHE BOOL "Disable Qt support in cucumber-cpp")
set(CUKE_TESTS_UNIT OFF CACHE BOOL "Disable cucumber-cpp unit tests")
set(CUKE_TESTS_E2E OFF CACHE BOOL "Disable cucumber-cpp e2e tests")
set(CUKE_ENABLE_GTEST ON CACHE BOOL "Enable GTest backend")
set(CUKE_ENABLE_BOOST_TEST OFF CACHE BOOL "Disable Boost.Test backend")
set(CUKE_ENABLE_EXAMPLES OFF CACHE BOOL "Disable cucumber-cpp examples")

# Disable cucumber-cpp install targets to avoid gtest export conflicts
set(CMAKE_SKIP_INSTALL_RULES ON)

# Disable warnings-as-errors for dependencies to avoid GCC 12+ false positives
# with gtest internal headers (char_traits.h)
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -Wno-error=restrict")

# Fetch cucumber-cpp
FetchContent_Declare(
    cucumber-cpp
    GIT_REPOSITORY https://github.com/cucumber/cucumber-cpp.git
    GIT_TAG        v0.7.0
)
FetchContent_MakeAvailable(cucumber-cpp)

# Handler sources from aggregates (compiled into tests since no library targets exist)
set(PLAYER_HANDLER_SOURCES
    ${CMAKE_SOURCE_DIR}/player/agg/src/player_state.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/register_handler.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/deposit_handler.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/withdraw_handler.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/reserve_handler.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/release_handler.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/transfer_handler.cpp
    ${CMAKE_SOURCE_DIR}/player/agg/handlers/rejected_handler.cpp
)

set(TABLE_HANDLER_SOURCES
    ${CMAKE_SOURCE_DIR}/table/agg/src/table_state.cpp
    ${CMAKE_SOURCE_DIR}/table/agg/handlers/create_handler.cpp
    ${CMAKE_SOURCE_DIR}/table/agg/handlers/join_handler.cpp
    ${CMAKE_SOURCE_DIR}/table/agg/handlers/leave_handler.cpp
    ${CMAKE_SOURCE_DIR}/table/agg/handlers/start_hand_handler.cpp
    ${CMAKE_SOURCE_DIR}/table/agg/handlers/end_hand_handler.cpp
)

set(HAND_HANDLER_SOURCES
    ${CMAKE_SOURCE_DIR}/hand/agg/src/hand_state.cpp
    ${CMAKE_SOURCE_DIR}/hand/agg/handlers/deal_handler.cpp
    ${CMAKE_SOURCE_DIR}/hand/agg/handlers/post_blind_handler.cpp
    ${CMAKE_SOURCE_DIR}/hand/agg/handlers/action_handler.cpp
    ${CMAKE_SOURCE_DIR}/hand/agg/handlers/deal_community_handler.cpp
    ${CMAKE_SOURCE_DIR}/hand/agg/handlers/award_pot_handler.cpp
)

# Step definition sources - compiled as single unit to avoid __COUNTER__ collisions
# between translation units (cucumber-cpp uses __COUNTER__ for unique class names)
set(STEP_SOURCES
    all_steps.cpp
)

# Create test executable
add_executable(acceptance_tests
    runner.cpp
    ${STEP_SOURCES}
    ${PLAYER_HANDLER_SOURCES}
    ${TABLE_HANDLER_SOURCES}
    ${HAND_HANDLER_SOURCES}
)

target_include_directories(acceptance_tests PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_SOURCE_DIR}/player/agg/src
    ${CMAKE_SOURCE_DIR}/player/agg/handlers
    ${CMAKE_SOURCE_DIR}/table/agg/src
    ${CMAKE_SOURCE_DIR}/table/agg/handlers
    ${CMAKE_SOURCE_DIR}/hand/agg/src
    ${CMAKE_SOURCE_DIR}/hand/agg/handlers
)

target_link_libraries(acceptance_tests PRIVATE
    cucumber-cpp
    GTest::gtest
    GTest::gtest_main
    Boost::regex
    Boost::filesystem
    Boost::system
    Boost::program_options
    Boost::thread
    nlohmann_json::nlohmann_json
    angzarr_client
    angzarr_proto
)

# Disable -Werror=restrict for gtest headers which trigger false positives
# in gtest type-util.h with GCC 12+ and certain optimization levels
target_compile_options(acceptance_tests PRIVATE
    -Wno-error=restrict
)

# Feature files location
set(FEATURE_DIR "${CMAKE_SOURCE_DIR}/features")

# Add CTest integration with acceptance label
add_test(
    NAME acceptance_tests
    COMMAND acceptance_tests
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
)
set_tests_properties(acceptance_tests PROPERTIES
    LABELS "acceptance"
    ENVIRONMENT "CUCUMBER_FEATURES_DIR=${FEATURE_DIR}"
)

# Custom target for running acceptance tests
add_custom_target(test-acceptance
    COMMAND ${CMAKE_CTEST_COMMAND} -L acceptance --output-on-failure
    DEPENDS acceptance_tests
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    COMMENT "Running acceptance tests"
)
