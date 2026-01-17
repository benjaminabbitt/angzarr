add_executable(projector-receipt
    src/main.cc
    src/projector_receipt.cc
)

target_include_directories(projector-receipt PRIVATE include)
target_link_libraries(projector-receipt PRIVATE angzarr_common)
