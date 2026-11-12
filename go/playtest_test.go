package playtest_test

import (
	"regexp"
	"testing"

	playtest "github.com/Archont561/playtest/go"
)

var semverPattern = regexp.MustCompile(`^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$`)

func TestVersionAccessible(t *testing.T) {
	if !semverPattern.MatchString(playtest.Version) {
		t.Fatalf("expected valid semver version, got %q", playtest.Version)
	}
}
