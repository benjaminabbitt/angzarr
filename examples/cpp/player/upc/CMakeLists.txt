add_executable(player-upc src/main.cpp)

target_link_libraries(player-upc PRIVATE
    angzarr_proto
    angzarr_client
    gRPC::grpc++_reflection
)
