# Playtest for C++

Polyglot test framework with the batteries included: dependency-injected fixtures, strict
LIFO teardown, property-based testing, and code-first BDD for C++.

## Installation

Through Conan:

```bash
conan install --requires=playtest-cpp/0.1.0
```

Or by adding the `cpp/` directory to your CMake build as a subdirectory.

## Quick Start
```cpp
#include <iostream>
#include <playtest/playtest.hpp>

int main() {
    std::cout << playtest::VERSION << "\n"; // 0.1.0
}
```

The header requires `PLAYTEST_CPP_VER` to be defined by the build system; the CMake target
sets it from the project version.

## Status

This package currently exposes the version contract and is wired into the shared CLI's
mirror and release pipeline. The fixture engine, code-first BDD, and property-based testing
land through [Milestone M5](https://github.com/Archont561/playtest/blob/main/backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md).
The README tracks the same contract as the other eight targets; it does not promise more
than has shipped.
