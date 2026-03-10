add_executable(prj-output-oo src/main.cpp)

target_link_libraries(prj-output-oo PRIVATE
    angzarr_proto
    angzarr_client
)
