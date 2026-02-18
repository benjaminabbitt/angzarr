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
    ${PROTO_DIR}/examples/types.proto
    ${PROTO_DIR}/examples/player.proto
    ${PROTO_DIR}/examples/table.proto
    ${PROTO_DIR}/examples/hand.proto
)

set(GENERATED_DIR "${CMAKE_BINARY_DIR}/generated")
file(MAKE_DIRECTORY ${GENERATED_DIR})

# Generate protobuf and gRPC sources
set(PROTO_SRCS)
set(PROTO_HDRS)
set(GRPC_SRCS)
set(GRPC_HDRS)

foreach(PROTO_FILE ${PROTO_FILES})
    get_filename_component(PROTO_NAME ${PROTO_FILE} NAME_WE)
    get_filename_component(PROTO_PATH ${PROTO_FILE} DIRECTORY)

    list(APPEND PROTO_SRCS "${GENERATED_DIR}/${PROTO_NAME}.pb.cc")
    list(APPEND PROTO_HDRS "${GENERATED_DIR}/${PROTO_NAME}.pb.h")
    list(APPEND GRPC_SRCS "${GENERATED_DIR}/${PROTO_NAME}.grpc.pb.cc")
    list(APPEND GRPC_HDRS "${GENERATED_DIR}/${PROTO_NAME}.grpc.pb.h")

    add_custom_command(
        OUTPUT "${GENERATED_DIR}/${PROTO_NAME}.pb.cc"
               "${GENERATED_DIR}/${PROTO_NAME}.pb.h"
               "${GENERATED_DIR}/${PROTO_NAME}.grpc.pb.cc"
               "${GENERATED_DIR}/${PROTO_NAME}.grpc.pb.h"
        COMMAND protobuf::protoc
        ARGS --grpc_out=${GENERATED_DIR}
             --cpp_out=${GENERATED_DIR}
             --plugin=protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>
             -I${PROTO_DIR}
             ${PROTO_FILE}
        DEPENDS ${PROTO_FILE}
        COMMENT "Generating C++ protobuf/gRPC sources for ${PROTO_NAME}"
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
