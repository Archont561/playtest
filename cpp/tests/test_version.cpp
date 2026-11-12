#include <cassert>
#include <iostream>
#include <playtest/playtest.hpp>

int main() {
  assert(!playtest::VERSION.empty());
  assert(playtest::VERSION.find('.') != std::string_view::npos);
  std::cout << "test_version passed: " << playtest::VERSION << "\n";
  return 0;
}
