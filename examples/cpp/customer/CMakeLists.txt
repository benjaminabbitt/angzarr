add_executable(customer
    src/main.cc
    src/customer_logic.cc
    src/customer_service.cc
)

target_include_directories(customer PRIVATE include)
target_link_libraries(customer PRIVATE angzarr_common)
