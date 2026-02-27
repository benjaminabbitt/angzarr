add_executable(saga-table-hand-oo src/main.cpp)

target_link_libraries(saga-table-hand-oo PRIVATE
    angzarr_proto
    angzarr_client
)
