add_executable(saga-table-player
    src/main.cpp
    src/table_player_saga.cpp
)

target_include_directories(saga-table-player PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(saga-table-player PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
