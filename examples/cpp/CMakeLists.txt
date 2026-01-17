cmake_minimum_required(VERSION 3.20)
project(angzarr_cpp_examples VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Find required packages
find_package(Protobuf REQUIRED)
find_package(gRPC CONFIG REQUIRED)
find_package(nlohmann_json 3.11 REQUIRED)

# Get gRPC plugin location
get_target_property(gRPC_CPP_PLUGIN_EXECUTABLE gRPC::grpc_cpp_plugin LOCATION)

# Proto directories
set(PROTO_DIR "${CMAKE_SOURCE_DIR}/../../proto")
set(GENERATED_DIR "${CMAKE_BINARY_DIR}/generated")

file(MAKE_DIRECTORY ${GENERATED_DIR})

# Generate proto files
set(ANGZARR_PROTO "${PROTO_DIR}/angzarr/angzarr.proto")
set(DOMAINS_PROTO "${PROTO_DIR}/examples/domains.proto")

# Custom command to generate protobuf and grpc files
add_custom_command(
    OUTPUT
        ${GENERATED_DIR}/angzarr.pb.cc
        ${GENERATED_DIR}/angzarr.pb.h
        ${GENERATED_DIR}/angzarr.grpc.pb.cc
        ${GENERATED_DIR}/angzarr.grpc.pb.h
    COMMAND ${Protobuf_PROTOC_EXECUTABLE}
        --proto_path=${PROTO_DIR}
        --cpp_out=${GENERATED_DIR}
        --grpc_out=${GENERATED_DIR}
        --plugin=protoc-gen-grpc=${gRPC_CPP_PLUGIN_EXECUTABLE}
        ${ANGZARR_PROTO}
    DEPENDS ${ANGZARR_PROTO}
)

add_custom_command(
    OUTPUT
        ${GENERATED_DIR}/domains.pb.cc
        ${GENERATED_DIR}/domains.pb.h
    COMMAND ${Protobuf_PROTOC_EXECUTABLE}
        --proto_path=${PROTO_DIR}
        --cpp_out=${GENERATED_DIR}
        ${DOMAINS_PROTO}
    DEPENDS ${DOMAINS_PROTO}
)

# Proto library
add_library(angzarr_proto
    ${GENERATED_DIR}/angzarr.pb.cc
    ${GENERATED_DIR}/angzarr.grpc.pb.cc
    ${GENERATED_DIR}/domains.pb.cc
)

target_include_directories(angzarr_proto PUBLIC ${GENERATED_DIR})
target_link_libraries(angzarr_proto PUBLIC
    protobuf::libprotobuf
    gRPC::grpc++
)

# Common library for shared utilities
add_library(angzarr_common INTERFACE)
target_include_directories(angzarr_common INTERFACE ${CMAKE_SOURCE_DIR}/common/include)
target_link_libraries(angzarr_common INTERFACE
    angzarr_proto
    nlohmann_json::nlohmann_json
)

# Add subdirectories for each service
add_subdirectory(customer)
add_subdirectory(product)
add_subdirectory(cart)
add_subdirectory(order)
add_subdirectory(inventory)
add_subdirectory(fulfillment)
add_subdirectory(saga-fulfillment)
add_subdirectory(saga-loyalty-earn)
add_subdirectory(saga-cancellation)
add_subdirectory(projector-receipt)
