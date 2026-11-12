"""Conan 2 recipe for the Playtest C static library."""

from pathlib import Path
import re

from conan import ConanFile
from conan.tools.cmake import CMake, cmake_layout
from conan.tools.files import load


class PlaytestCConan(ConanFile):
    name = "playtest-c"
    package_type = "static-library"
    settings = "os", "arch", "compiler", "build_type"
    license = "MIT"
    author = "Archont561"
    url = "https://github.com/Archont561/playtest"
    description = "Playwright-style testing framework for C"
    topics = ("testing", "bdd", "property-based-testing")
    exports_sources = "CMakeLists.txt", "include/**", "src/**", "LICENSE*"

    def set_version(self):
        cmake = load(self, str(Path(self.recipe_folder) / "CMakeLists.txt"))
        match = re.search(r"project\([^\)]*VERSION\s+([0-9A-Za-z.+-]+)", cmake)
        if not match:
            raise RuntimeError("Missing project(... VERSION ...) in c/CMakeLists.txt")
        self.version = match.group(1)

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
        self.cpp_info.defines = [f"PLAYTEST_C_VER={self.version}"]
