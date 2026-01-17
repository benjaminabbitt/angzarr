add_executable(cart
    src/main.cc
    src/cart_logic.cc
    src/cart_service.cc
)

target_include_directories(cart PRIVATE include)
target_link_libraries(cart PRIVATE angzarr_common)
