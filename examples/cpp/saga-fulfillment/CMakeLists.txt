add_executable(saga-fulfillment
    src/main.cc
    src/saga_fulfillment.cc
)

target_include_directories(saga-fulfillment PRIVATE include)
target_link_libraries(saga-fulfillment PRIVATE angzarr_common)
