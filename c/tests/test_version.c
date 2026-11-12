#include <assert.h>
#include <playtest/playtest.h>
#include <stdio.h>
#include <string.h>

int main(void) {
  assert(strlen(PLAYTEST_VERSION) >= 5);
  assert(strchr(PLAYTEST_VERSION, '.') != NULL);
  assert(strcmp(playtest_version(), PLAYTEST_VERSION) == 0);
  printf("C test_version passed: %s\n", playtest_version());
  return 0;
}
