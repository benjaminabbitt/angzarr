cmake_minimum_required(VERSION 3.20)
project(angzarr_examples_cpp VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Find required packages
find_package(Protobuf REQUIRED)
find_package(gRPC CONFIG REQUIRED)
find_package(absl CONFIG REQUIRED)

# Proto files location
set(PROTO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../client/rust/proto")
set(ANGZARR_PROTO_DIR "${PROTO_ROOT}/angzarr")
set(EXAMPLES_PROTO_DIR "${PROTO_ROOT}/examples")

# Generated proto output
set(PROTO_GEN_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY ${PROTO_GEN_DIR})

# Collect proto files
file(GLOB ANGZARR_PROTOS "${ANGZARR_PROTO_DIR}/*.proto")
file(GLOB EXAMPLES_PROTOS "${EXAMPLES_PROTO_DIR}/*.proto")
set(ALL_PROTOS ${ANGZARR_PROTOS} ${EXAMPLES_PROTOS})

# Generate proto sources
set(PROTO_SRCS)
set(PROTO_HDRS)
set(GRPC_SRCS)
set(GRPC_HDRS)

foreach(PROTO_FILE ${ALL_PROTOS})
    get_filename_component(PROTO_NAME ${PROTO_FILE} NAME_WE)
    get_filename_component(PROTO_DIR ${PROTO_FILE} DIRECTORY)
    file(RELATIVE_PATH PROTO_REL_DIR ${PROTO_ROOT} ${PROTO_DIR})

    set(PROTO_SRC "${PROTO_GEN_DIR}/${PROTO_REL_DIR}/${PROTO_NAME}.pb.cc")
    set(PROTO_HDR "${PROTO_GEN_DIR}/${PROTO_REL_DIR}/${PROTO_NAME}.pb.h")
    set(GRPC_SRC "${PROTO_GEN_DIR}/${PROTO_REL_DIR}/${PROTO_NAME}.grpc.pb.cc")
    set(GRPC_HDR "${PROTO_GEN_DIR}/${PROTO_REL_DIR}/${PROTO_NAME}.grpc.pb.h")

    list(APPEND PROTO_SRCS ${PROTO_SRC})
    list(APPEND PROTO_HDRS ${PROTO_HDR})
    list(APPEND GRPC_SRCS ${GRPC_SRC})
    list(APPEND GRPC_HDRS ${GRPC_HDR})

    add_custom_command(
        OUTPUT ${PROTO_SRC} ${PROTO_HDR} ${GRPC_SRC} ${GRPC_HDR}
        COMMAND ${Protobuf_PROTOC_EXECUTABLE}
        ARGS --proto_path=${PROTO_ROOT}
             --cpp_out=${PROTO_GEN_DIR}
             --grpc_out=${PROTO_GEN_DIR}
             --plugin=protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>
             ${PROTO_FILE}
        DEPENDS ${PROTO_FILE}
        COMMENT "Generating C++ sources from ${PROTO_NAME}.proto"
    )
endforeach()

# Proto library
add_library(angzarr_proto STATIC ${PROTO_SRCS} ${GRPC_SRCS})
target_include_directories(angzarr_proto PUBLIC ${PROTO_GEN_DIR})
target_link_libraries(angzarr_proto PUBLIC
    protobuf::libprotobuf
    gRPC::grpc++
)

# Client library
add_library(angzarr_client INTERFACE)
target_include_directories(angzarr_client INTERFACE
    ${CMAKE_CURRENT_SOURCE_DIR}/client/include
)
target_link_libraries(angzarr_client INTERFACE
    angzarr_proto
)

# Subdirectories
add_subdirectory(player/agg)
add_subdirectory(table/agg)
add_subdirectory(hand/agg)
add_subdirectory(table/saga-hand)
add_subdirectory(table/saga-player)
add_subdirectory(hand/saga-table)
add_subdirectory(hand/saga-player)
add_subdirectory(hand-flow)
add_subdirectory(prj-output)

# Tests (optional, requires cucumber-cpp)
option(BUILD_TESTS "Build cucumber-cpp tests" OFF)
if(BUILD_TESTS)
    add_subdirectory(tests)
endif()
