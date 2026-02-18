add_executable(pmg-hand-flow
    src/main.cpp
    src/hand_process.cpp
)

target_include_directories(pmg-hand-flow PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(pmg-hand-flow PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
