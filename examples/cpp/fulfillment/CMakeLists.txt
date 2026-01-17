add_executable(fulfillment
    src/main.cc
    src/fulfillment_logic.cc
    src/fulfillment_service.cc
)

target_include_directories(fulfillment PRIVATE include)
target_link_libraries(fulfillment PRIVATE angzarr_common)
