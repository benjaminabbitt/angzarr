add_executable(prj-output
    src/main.cpp
    src/output_projector.cpp
    src/text_renderer.cpp
)

target_include_directories(prj-output PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(prj-output PRIVATE
    angzarr_client
    gRPC::grpc++_reflection
)

# Compiler flags for potential warnings
target_compile_options(prj-output PRIVATE -Wall -Wextra)
