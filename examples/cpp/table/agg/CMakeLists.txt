add_executable(agg-table
    src/main.cpp
    src/table_state.cpp
    src/table.cpp
    handlers/create_handler.cpp
    handlers/join_handler.cpp
    handlers/leave_handler.cpp
    handlers/start_hand_handler.cpp
    handlers/end_hand_handler.cpp
)

target_include_directories(agg-table PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/handlers
)

target_link_libraries(agg-table PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
