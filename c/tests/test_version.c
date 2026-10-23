#include <assert.h>
#include <playtest/playtest.h>
#include <stdio.h>
#include <string.h>

int main(void) {
  assert(strcmp(PLAYTEST_VERSION, "0.1.0") == 0);
  assert(strcmp(playtest_version(), "0.1.0") == 0);
  printf("C test_version passed: %s\n", playtest_version());
  return 0;
}
