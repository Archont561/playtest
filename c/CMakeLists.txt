cmake_minimum_required(VERSION 3.20)
project(playtest-c VERSION 0.1.0 LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

add_library(playtest STATIC src/playtest.c)
target_include_directories(playtest PUBLIC include)
target_compile_definitions(playtest PUBLIC PLAYTEST_C_VER=${PROJECT_VERSION})

enable_testing()
add_executable(test_version tests/test_version.c)
target_link_libraries(test_version PRIVATE playtest)
add_test(NAME test_version COMMAND test_version)

include(GNUInstallDirs)
install(TARGETS playtest
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(DIRECTORY include/ DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
