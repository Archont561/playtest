//
// Playtest for C++: dependency-injected fixtures with strict LIFO teardown,
// property-based testing, and code-first BDD sharing one contract across nine
// language targets.
//

#pragma once

#include <string_view>

#ifndef PLAYTEST_CPP_VER
#error "PLAYTEST_CPP_VER must be defined by CMake or the build system"
#endif

#define PLAYTEST_STR_IMPL(x) #x
#define PLAYTEST_STR(x) PLAYTEST_STR_IMPL(x)

namespace playtest {

inline constexpr std::string_view VERSION = PLAYTEST_STR(PLAYTEST_CPP_VER);

} // namespace playtest
