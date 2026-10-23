cmake_minimum_required(VERSION 3.20)
project(playtest-cpp VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(playtest INTERFACE)
target_include_directories(playtest INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include>
)

enable_testing()
add_executable(test_version tests/test_version.cpp)
target_link_libraries(test_version PRIVATE playtest)
add_test(NAME test_version COMMAND test_version)
