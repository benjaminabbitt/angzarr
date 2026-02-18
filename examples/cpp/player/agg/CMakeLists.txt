add_executable(agg-player
    src/main.cpp
    src/player_state.cpp
    src/player.cpp
    handlers/register_handler.cpp
    handlers/deposit_handler.cpp
    handlers/withdraw_handler.cpp
    handlers/reserve_handler.cpp
    handlers/release_handler.cpp
    handlers/transfer_handler.cpp
)

target_include_directories(agg-player PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_CURRENT_SOURCE_DIR}/handlers
)

target_link_libraries(agg-player PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
