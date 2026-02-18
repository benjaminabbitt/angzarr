add_executable(saga-hand-player
    src/main.cpp
    src/hand_player_saga.cpp
)

target_include_directories(saga-hand-player PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(saga-hand-player PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)
