cmake_minimum_required(VERSION 3.20)
project(angzarr-client VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Find required packages
find_package(Protobuf REQUIRED)
find_package(gRPC CONFIG REQUIRED)
find_package(absl CONFIG REQUIRED)

# Proto generation
set(PROTO_DIR "${CMAKE_SOURCE_DIR}/../../proto")
set(PROTO_FILES
    ${PROTO_DIR}/angzarr/types.proto
    ${PROTO_DIR}/angzarr/aggregate.proto
    ${PROTO_DIR}/angzarr/saga.proto
    ${PROTO_DIR}/angzarr/projector.proto
    ${PROTO_DIR}/angzarr/process_manager.proto
    ${PROTO_DIR}/angzarr/query.proto
    ${PROTO_DIR}/angzarr/stream.proto
    ${PROTO_DIR}/angzarr/upcaster.proto
    ${PROTO_DIR}/angzarr/meta.proto
    ${PROTO_DIR}/examples/poker_types.proto
    ${PROTO_DIR}/examples/player.proto
    ${PROTO_DIR}/examples/table.proto
    ${PROTO_DIR}/examples/hand.proto
)

set(GENERATED_DIR "${CMAKE_BINARY_DIR}/generated")
file(MAKE_DIRECTORY ${GENERATED_DIR})
file(MAKE_DIRECTORY ${GENERATED_DIR}/angzarr)
file(MAKE_DIRECTORY ${GENERATED_DIR}/examples)

# Generate protobuf and gRPC sources
# Use relative path from PROTO_DIR to preserve subdirectory structure
set(PROTO_SRCS)
set(PROTO_HDRS)
set(GRPC_SRCS)
set(GRPC_HDRS)

foreach(PROTO_FILE ${PROTO_FILES})
    get_filename_component(PROTO_NAME ${PROTO_FILE} NAME_WE)
    # Get relative path from PROTO_DIR to preserve subdirectory (angzarr/ or examples/)
    file(RELATIVE_PATH PROTO_REL_PATH ${PROTO_DIR} ${PROTO_FILE})
    get_filename_component(PROTO_SUBDIR ${PROTO_REL_PATH} DIRECTORY)

    set(OUT_DIR "${GENERATED_DIR}/${PROTO_SUBDIR}")
    file(MAKE_DIRECTORY ${OUT_DIR})

    list(APPEND PROTO_SRCS "${OUT_DIR}/${PROTO_NAME}.pb.cc")
    list(APPEND PROTO_HDRS "${OUT_DIR}/${PROTO_NAME}.pb.h")
    list(APPEND GRPC_SRCS "${OUT_DIR}/${PROTO_NAME}.grpc.pb.cc")
    list(APPEND GRPC_HDRS "${OUT_DIR}/${PROTO_NAME}.grpc.pb.h")

    add_custom_command(
        OUTPUT "${OUT_DIR}/${PROTO_NAME}.pb.cc"
               "${OUT_DIR}/${PROTO_NAME}.pb.h"
               "${OUT_DIR}/${PROTO_NAME}.grpc.pb.cc"
               "${OUT_DIR}/${PROTO_NAME}.grpc.pb.h"
        COMMAND protobuf::protoc
        ARGS --grpc_out=${GENERATED_DIR}
             --cpp_out=${GENERATED_DIR}
             --plugin=protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>
             -I${PROTO_DIR}
             ${PROTO_FILE}
        DEPENDS ${PROTO_FILE}
        COMMENT "Generating C++ protobuf/gRPC sources for ${PROTO_SUBDIR}/${PROTO_NAME}"
    )
endforeach()

# Client library
add_library(angzarr-client
    src/helpers.cpp
    src/validation.cpp
    src/router.cpp
    src/aggregate.cpp
    ${PROTO_SRCS}
    ${GRPC_SRCS}
)

target_include_directories(angzarr-client
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/include>
        $<BUILD_INTERFACE:${GENERATED_DIR}>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(angzarr-client
    PUBLIC
        protobuf::libprotobuf
        gRPC::grpc++
        absl::strings
)

# Install
install(TARGETS angzarr-client
    EXPORT angzarr-client-targets
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
    INCLUDES DESTINATION include
)

install(DIRECTORY include/angzarr
    DESTINATION include
)

install(DIRECTORY ${GENERATED_DIR}/
    DESTINATION include
    FILES_MATCHING PATTERN "*.h"
)

# Testing
enable_testing()
find_package(GTest CONFIG REQUIRED)

add_executable(rejection_handler_response_test
    tests/rejection_handler_response_test.cpp
)

target_link_libraries(rejection_handler_response_test
    PRIVATE
        angzarr-client
        GTest::gtest
        GTest::gtest_main
)

add_executable(builder_test
    tests/builder_test.cpp
)

target_link_libraries(builder_test
    PRIVATE
        angzarr-client
        GTest::gtest
        GTest::gtest_main
)

add_executable(error_introspection_test
    tests/error_introspection_test.cpp
)

target_link_libraries(error_introspection_test
    PRIVATE
        angzarr-client
        GTest::gtest
        GTest::gtest_main
)

add_executable(router_test
    tests/router_test.cpp
)

target_link_libraries(router_test
    PRIVATE
        angzarr-client
        GTest::gtest
        GTest::gtest_main
)

add_executable(compensation_test
    tests/compensation_test.cpp
)

target_link_libraries(compensation_test
    PRIVATE
        angzarr-client
        GTest::gtest
        GTest::gtest_main
)

add_executable(state_building_test
    tests/state_building_test.cpp
)

target_link_libraries(state_building_test
    PRIVATE
        angzarr-client
        GTest::gtest
        GTest::gtest_main
)

include(GoogleTest)
gtest_discover_tests(rejection_handler_response_test)
gtest_discover_tests(builder_test)
gtest_discover_tests(error_introspection_test)
gtest_discover_tests(router_test)
gtest_discover_tests(compensation_test)
gtest_discover_tests(state_building_test)
