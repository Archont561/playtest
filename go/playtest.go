// Package playtest is a polyglot test framework with the batteries included:
// dependency-injected fixtures, strict LIFO teardown, property-based testing, and
// code-first BDD for Go.
package playtest

import (
	_ "embed"
	"strings"
)

//go:embed VERSION
var rawVersion string

// Version of the playtest package, discovered from the embedded VERSION manifest.
var Version = strings.TrimSpace(rawVersion)
