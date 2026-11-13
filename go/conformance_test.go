package playtest_test

// Cross-language conformance suites for the fixture contract (Spec 0005 §1, task_023).
//
// Each test is one cell of the `(capability x target)` matrix in `backlog/conformance.yml`.
// The capabilities exercised here are the ones this target currently claims; the others
// are declared in the manifest but not claimed, so they report as `not_implemented`
// rather than as passing.
//
// The tests assert observable behaviour through the public API only, so the same scenarios
// are expressible in the Rust, Python, and TypeScript suites without sharing
// implementation details.

import (
	"fmt"
	"testing"

	playtest "github.com/Archont561/playtest/go"
)

// resolve returns a fixture constructor that records its construction now and its
// teardown when the stack reaches the hook.
func resolve(trace *[]string, name string) func() (string, playtest.TeardownFunc) {
	return func() (string, playtest.TeardownFunc) {
		*trace = append(*trace, name+":setup")
		return name, func() {
			*trace = append(*trace, name+":teardown")
		}
	}
}

// fixture_explicit_resolution: only requested fixtures are constructed, in request order.
func TestConformanceFixtureExplicitResolution(t *testing.T) {
	var trace []string
	stack := playtest.NewFixtureStack()

	// resolve returns a constructor; declaring one without passing it to Setup means the
	// fixture exists but is never resolved, so nothing about it is constructed.
	_ = resolve(&trace, "unused")

	playtest.Setup(stack, resolve(&trace, "requested"))

	if got, want := fmt.Sprint(trace), "[requested:setup]"; got != want {
		t.Errorf("construction trace = %v, want %v", got, want)
	}
	if stack.Len() != 1 {
		t.Errorf("only the requested fixture is held, got %d", stack.Len())
	}
}

// teardown_lifo: cleanup runs in strict reverse order of successful creation.
func TestConformanceTeardownLIFO(t *testing.T) {
	var trace []string
	stack := playtest.NewFixtureStack()
	for _, name := range []string{"a", "b", "c"} {
		playtest.Setup(stack, resolve(&trace, name))
	}

	report := stack.TeardownAll()

	if !report.IsClean() {
		t.Errorf("teardown reported failures: %v", report.Failures)
	}
	if report.Ran != 3 {
		t.Errorf("ran %d hooks, want 3", report.Ran)
	}
	want := "[a:setup b:setup c:setup c:teardown b:teardown a:teardown]"
	if got := fmt.Sprint(trace); got != want {
		t.Errorf("teardown must be the exact reverse of setup:\n got %v\nwant %v", got, want)
	}
}

// teardown_failure_safety: a panicking cleanup never stops the remaining cleanups.
func TestConformanceTeardownFailureSafety(t *testing.T) {
	var trace []string
	stack := playtest.NewFixtureStack()
	playtest.Setup(stack, resolve(&trace, "outer"))

	stack.Push(func() {
		trace = append(trace, "boom:teardown")
		panic("deliberate conformance failure")
	})

	report := stack.TeardownAll()

	if report.IsClean() {
		t.Error("the failing cleanup must be reported")
	}
	if len(report.Failures) != 1 {
		t.Errorf("recorded %d failures, want 1", len(report.Failures))
	}
	if !contains(trace, "boom:teardown") {
		t.Error("the failing fixture's cleanup ran")
	}
	if !contains(trace, "outer:teardown") {
		t.Error("the fixture created before the failure must still be released")
	}
}

// The harness must observe a panic from the body *and* still run teardown.
func TestConformanceBodyPanicStillTearsDown(t *testing.T) {
	var trace []string
	stack := playtest.NewFixtureStack()
	playtest.Setup(stack, resolve(&trace, "held"))

	panicked := didPanic(func() {
		playtest.Run(stack, func() {
			panic("body failure")
		})
	})

	if !panicked {
		t.Error("the body's panic is surfaced")
	}
	if stack.Len() != 0 {
		t.Errorf("every fixture was released, %d left", stack.Len())
	}
	if !contains(trace, "held:teardown") {
		t.Error("the fixture was torn down despite the panic")
	}
}

// The crate exposes a version, which the parity report uses to prove the target is real.
func TestConformanceVersionIsNonEmpty(t *testing.T) {
	if playtest.Version == "" {
		t.Error("Version must not be empty")
	}
}

// didPanic reports whether fn panicked, absorbing the panic so the test can assert on it.
func didPanic(fn func()) (panicked bool) {
	defer func() {
		if recover() != nil {
			panicked = true
		}
	}()
	fn()
	return false
}

// contains reports whether want appears in the trace.
func contains(trace []string, want string) bool {
	for _, entry := range trace {
		if entry == want {
			return true
		}
	}
	return false
}
