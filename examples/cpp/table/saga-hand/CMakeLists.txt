add_executable(saga-table-hand
    src/main.cpp
    src/table_hand_saga.cpp
)

target_include_directories(saga-table-hand PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(saga-table-hand PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
