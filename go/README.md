# Playtest for Go

Polyglot test framework with the batteries included: dependency-injected fixtures, strict
LIFO teardown, property-based testing, and code-first BDD for Go.

## Installation
```bash
go get github.com/Archont561/playtest/go
```

## Quick Start
```go
package main

import (
	"fmt"

	playtest "github.com/Archont561/playtest/go"
)

func main() {
	fmt.Println(playtest.Version) // 0.1.0
}
```

## Fixtures

```go
func TestReadsConfig(t *testing.T) {
	playtest.RunT(t, func(stack *playtest.FixtureStack) {
		dir := playtest.Setup(stack, func() (string, playtest.TeardownFunc) {
			path, err := os.MkdirTemp("", "playtest-")
			if err != nil {
				t.Fatal(err)
			}
			return path, func() { os.RemoveAll(path) }
		})
		// dir is removed after the test, even if an assertion below fails.
		_ = dir
	})
}
```

Teardown is strict LIFO: fixtures are released in the reverse of the order they were
resolved, and one failing cleanup does not prevent the others.

## Skipping

```go
func TestRendering(t *testing.T) {
	if playtest.SkipIf(t, !haveGPU(), "gpu rendering", "requires a GPU") {
		return
	}
}
```

`SkipNow` records the skip and then calls `t.Skip`, so `go test` reports it the way Go
users expect *and* Playtest holds an explicit outcome. A skip is never reported as a pass.

## Status

This package implements the fixture core of [Spec 0005 §1.1](https://github.com/Archont561/playtest/blob/main/backlog/docs/specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md)
and the skip outcome of §1.4: explicit resolution, strict LIFO teardown, failure-safe
teardown, and skip reporting are implemented and covered by `conformance_test.go`, so the
Go target claims those four capabilities in the cross-language parity matrix.

Code-first BDD and property-based testing land through
[Milestone M5](https://github.com/Archont561/playtest/blob/main/backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md).
The README tracks the same contract as the other eight targets; it does not promise more
than has shipped.
