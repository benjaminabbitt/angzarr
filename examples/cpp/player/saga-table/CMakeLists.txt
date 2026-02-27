add_executable(saga-player-table
    src/main.cpp
    src/player_table_saga.cpp
)

target_include_directories(saga-player-table PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(saga-player-table PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
