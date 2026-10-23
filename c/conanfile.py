"""Conan 2 recipe for the Playtest C static library."""

from conan import ConanFile
from conan.tools.cmake import CMake, cmake_layout


class PlaytestCConan(ConanFile):
    name = "playtest-c"
    version = "0.1.0"
    package_type = "static-library"
    settings = "os", "arch", "compiler", "build_type"
    license = "MIT OR Apache-2.0"
    author = "Archont561"
    url = "https://github.com/Archont561/playtest"
    description = "Playwright-style testing framework for C"
    topics = ("testing", "bdd", "property-based-testing")
    exports_sources = "CMakeLists.txt", "include/**", "src/**", "LICENSE*"

    def layout(self):
        cmake_layout(self)

    def build(self):
        cmake = CMake(self)
        cmake.configure()
        cmake.build()

    def package(self):
        cmake = CMake(self)
        cmake.install()

    def package_info(self):
        self.cpp_info.libs = ["playtest"]
        self.cpp_info.set_property("cmake_file_name", "playtest-c")
        self.cpp_info.set_property("cmake_target_name", "playtest-c::playtest")
