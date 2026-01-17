add_executable(order
    src/main.cc
    src/order_logic.cc
    src/order_service.cc
)

target_include_directories(order PRIVATE include)
target_link_libraries(order PRIVATE angzarr_common)
