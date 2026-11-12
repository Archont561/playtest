// Package playtest provides Playwright-style testing fixtures and BDD scenarios.
package playtest

import (
	_ "embed"
	"strings"
)

//go:embed VERSION
var rawVersion string

// Version of the playtest package, discovered from the embedded VERSION manifest.
var Version = strings.TrimSpace(rawVersion)
