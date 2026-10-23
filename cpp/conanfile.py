"""Conan 2 recipe for the header-only Playtest C++ library."""

from conan import ConanFile
from conan.tools.files import copy
import os


class PlaytestCppConan(ConanFile):
    name = "playtest-cpp"
    version = "0.1.0"
    package_type = "header-library"
    license = "MIT OR Apache-2.0"
    author = "Archont561"
    url = "https://github.com/Archont561/playtest"
    description = "Playwright-style testing framework for C++"
    topics = ("testing", "bdd", "property-based-testing")
    exports_sources = "include/**", "LICENSE*"

    def package(self):
        copy(self, "*.hpp", src=os.path.join(self.source_folder, "include"),
             dst=os.path.join(self.package_folder, "include"), keep_path=True)

    def package_info(self):
        self.cpp_info.set_property("cmake_file_name", "playtest-cpp")
        self.cpp_info.set_property("cmake_target_name", "playtest-cpp::playtest-cpp")
        self.cpp_info.includedirs = ["include"]
