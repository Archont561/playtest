#include <cassert>
#include <iostream>
#include <playtest/playtest.hpp>

int main() {
  assert(playtest::VERSION == "0.1.0");
  std::cout << "test_version passed: " << playtest::VERSION << "\n";
  return 0;
}
