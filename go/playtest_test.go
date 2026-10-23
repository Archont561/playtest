package playtest_test

import (
	"testing"

	playtest "github.com/Archont561/playtest/go"
)

func TestVersionAccessible(t *testing.T) {
	expected := "0.1.0"
	if playtest.Version != expected {
		t.Fatalf("expected version %s, got %s", expected, playtest.Version)
	}
}
