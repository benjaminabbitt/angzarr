add_executable(inventory
    src/main.cc
    src/inventory_logic.cc
    src/inventory_service.cc
)

target_include_directories(inventory PRIVATE include)
target_link_libraries(inventory PRIVATE angzarr_common)
