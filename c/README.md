# Playtest for C

Playwright-style testing framework for C (C11/C23) with dependency-injected fixtures, strict LIFO teardown, property-based testing, and fluent BDD scenarios.

## Quick Start
```c
#include <assert.h>
#include <playtest/playtest.h>

int main(void) {
    assert(strcmp(playtest_version(), "0.1.0") == 0);
    return 0;
}
```
