# Playtest for C

Polyglot test framework with the batteries included: dependency-injected fixtures, strict LIFO teardown, property-based testing, and code-first BDD for C (C11/C23).

## Quick Start
```c
#include <assert.h>
#include <playtest/playtest.h>

int main(void) {
    assert(strcmp(playtest_version(), "0.1.0") == 0);
    return 0;
}
```
