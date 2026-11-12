"""Conan 2 recipe for the header-only Playtest C++ library."""

from pathlib import Path
import re

from conan import ConanFile
from conan.tools.files import copy, load


class PlaytestCppConan(ConanFile):
    name = "playtest-cpp"
    package_type = "header-library"
    license = "MIT"
    author = "Archont561"
    url = "https://github.com/Archont561/playtest"
    description = "Polyglot test framework with the batteries included: dependency-injected fixtures, strict LIFO teardown, property-based testing, and code-first BDD for C++"
    topics = ("testing", "bdd", "property-based-testing")
    exports_sources = "CMakeLists.txt", "include/**", "LICENSE*"

    def set_version(self):
        cmake = load(self, str(Path(self.recipe_folder) / "CMakeLists.txt"))
        match = re.search(r"project\([^\)]*VERSION\s+([0-9A-Za-z.+-]+)", cmake)
        if not match:
            raise RuntimeError("Missing project(... VERSION ...) in cpp/CMakeLists.txt")
        self.version = match.group(1)

    def package(self):
        copy(
            self,
            "*.hpp",
            src=str(Path(self.source_folder) / "include"),
            dst=str(Path(self.package_folder) / "include"),
            keep_path=True,
        )

    def package_info(self):
        self.cpp_info.set_property("cmake_file_name", "playtest-cpp")
        self.cpp_info.set_property("cmake_target_name", "playtest-cpp::playtest-cpp")
        self.cpp_info.includedirs = ["include"]
        self.cpp_info.defines = [f"PLAYTEST_CPP_VER={self.version}"]
