add_executable(saga-loyalty-earn
    src/main.cc
    src/saga_loyalty_earn.cc
)

target_include_directories(saga-loyalty-earn PRIVATE include)
target_link_libraries(saga-loyalty-earn PRIVATE angzarr_common)
