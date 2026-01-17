add_executable(saga-cancellation
    src/main.cc
    src/saga_cancellation.cc
)

target_include_directories(saga-cancellation PRIVATE include)
target_link_libraries(saga-cancellation PRIVATE angzarr_common)
