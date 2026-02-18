add_executable(pmg-hand-flow-oo
    src/main.cpp
)

target_include_directories(pmg-hand-flow-oo PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(pmg-hand-flow-oo PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
