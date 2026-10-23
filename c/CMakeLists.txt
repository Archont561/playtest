cmake_minimum_required(VERSION 3.20)
project(playtest C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

add_library(playtest STATIC src/playtest.c)
target_include_directories(playtest PUBLIC include)

enable_testing()
add_executable(test_version tests/test_version.c)
target_link_libraries(test_version PRIVATE playtest)
add_test(NAME test_version COMMAND test_version)
