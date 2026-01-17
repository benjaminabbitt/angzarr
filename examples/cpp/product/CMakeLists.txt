add_executable(product
    src/main.cc
    src/product_logic.cc
    src/product_service.cc
)

target_include_directories(product PRIVATE include)
target_link_libraries(product PRIVATE angzarr_common)
