package playtest

import "fmt"

// TeardownFunc releases one fixture. It is called exactly once, in reverse order of
// resolution, after the test body finishes.
//
// Teardown runs even when the test body fails. A teardown that panics does not stop the
// remaining teardowns: Go's testing package runs deferred functions while unwinding a
// panic, so the stack is released from a deferred call and its own panics are recorded
// rather than propagated.
type TeardownFunc func()

// New creates a teardown hook from a plain function. It exists so a fixture can be
// declared without naming the TeardownFunc type:
//
//	stack.Push(playtest.New(func() { os.RemoveAll(dir) }))
//
// It has no effect on ordering; the hook runs when the stack reaches it.
func New(fn func()) TeardownFunc {
	return TeardownFunc(fn)
}

// FixtureStack collects the teardown hooks of every fixture a test resolves and runs them
// in reverse resolution order (Spec 0001 §3.1, Spec 0005 §1.1.3).
//
// The ordering guarantee is the point. If a test resolves a, then b, then c, teardown runs
// c, b, a: a dependency can never observe a resource released before it.
//
// A FixtureStack is not safe for concurrent use. One test owns one stack.
type FixtureStack struct {
	hooks []TeardownFunc
}

// NewFixtureStack returns an empty stack.
func NewFixtureStack() *FixtureStack {
	return &FixtureStack{}
}

// Push registers a teardown hook. Hooks run in reverse registration order.
//
// Push order must match construction order: push the hook immediately after the fixture
// has been constructed successfully, so a failed construction does not leave a hook that
// tears down something that was never created.
func (s *FixtureStack) Push(hook TeardownFunc) {
	if hook == nil {
		return
	}
	s.hooks = append(s.hooks, hook)
}

// Len reports how many fixtures are still awaiting teardown.
func (s *FixtureStack) Len() int {
	return len(s.hooks)
}

// IsEmpty reports whether every fixture has been released.
func (s *FixtureStack) IsEmpty() bool {
	return len(s.hooks) == 0
}

// TeardownAll runs every registered hook in reverse registration order and reports what
// happened.
//
// A hook that panics is recovered and recorded, and the remaining hooks still run: one
// failing cleanup must never prevent the others (Spec 0005 §1.1.4). The stack is empty
// afterwards, so a second call is a no-op rather than a double release.
func (s *FixtureStack) TeardownAll() *TeardownReport {
	report := &TeardownReport{}
	for len(s.hooks) > 0 {
		last := len(s.hooks) - 1
		hook := s.hooks[last]
		s.hooks = s.hooks[:last]
		report.Ran++
		report.Failures = append(report.Failures, runHook(hook)...)
	}
	return report
}

// runHook calls one hook, converting a panic into a reported failure.
func runHook(hook TeardownFunc) (failures []string) {
	defer func() {
		if recovered := recover(); recovered != nil {
			failures = append(failures, panicMessage(recovered))
		}
	}()
	hook()
	return nil
}

// panicMessage renders a recovered panic value the way a reader expects to see it.
func panicMessage(recovered any) string {
	if text, ok := recovered.(string); ok {
		return text
	}
	if err, ok := recovered.(error); ok {
		return err.Error()
	}
	return fmt.Sprintf("%v", recovered)
}

// TeardownReport is what happened while releasing a test's fixtures.
type TeardownReport struct {
	// Ran is how many hooks were called.
	Ran int
	// Failures holds one message per hook that panicked, in the order the hooks ran.
	Failures []string
}

// IsClean reports whether every hook completed without panicking.
func (r *TeardownReport) IsClean() bool {
	return len(r.Failures) == 0
}

// TeardownError is returned by Run plus a non-clean report. It carries every failure so a
// consumer can tell which cleanup broke, instead of learning only that something did.
type TeardownError struct {
	// Report is the full teardown outcome.
	Report *TeardownReport
}

// Error implements the error interface.
func (e *TeardownError) Error() string {
	return fmt.Sprintf("playtest: %d teardown failure(s): %v", len(e.Report.Failures), e.Report.Failures)
}

// Err returns the report's failures as an error, or nil when the report is clean.
//
// The report is never reduced to a boolean: Spec 0005 §1.1.4 forbids hiding which cleanup
// failed, so the message carries every failure text.
func (r *TeardownReport) Err() error {
	if r.IsClean() {
		return nil
	}
	return &TeardownError{Report: r}
}

// Run executes body and then releases every fixture, returning the body's own outcome
// together with the teardown report (Spec 0001 §3.2).
//
// Teardown runs whether body returns normally or panics. When body panics, teardown is
// completed first and the panic is then re-raised, so the test still fails at the line
// that failed and the original failure is never replaced by a teardown message.
func Run(stack *FixtureStack, body func()) (report *TeardownReport) {
	defer func() {
		report = stack.TeardownAll()
	}()
	body()
	return nil
}

// RunT executes body against a fresh stack and fails the test when teardown reports a
// failure. This is the form most Go tests should use:
//
//	func TestUsesFixtures(t *testing.T) {
//		playtest.RunT(t, func(stack *playtest.FixtureStack) {
//			db := stack.Setup(newDatabase)
//			// ... assertions ...
//		})
//	}
//
// The body's own failure is reported by t itself. Teardown runs through t.Cleanup, which
// Go guarantees to call even when the test has already failed or panicked, so a failing
// assertion never skips cleanup (Spec 0005 §1.1.4).
func RunT(t TestingT, body func(stack *FixtureStack)) {
	stack := NewFixtureStack()
	t.Cleanup(func() {
		report := stack.TeardownAll()
		if err := report.Err(); err != nil {
			t.Errorf("%v", err)
		}
	})
	body(stack)
}

// TestingT is the part of *testing.T that RunT needs.
//
// It is an interface rather than a concrete *testing.T so the fixture engine stays usable
// outside the go test binary — and so the conformance suite can assert teardown behaviour
// without depending on the runner's failure bookkeeping.
type TestingT interface {
	// Cleanup registers a function to run when the test finishes.
	Cleanup(f func())
	// Errorf records a failure without stopping the test.
	Errorf(format string, args ...any)
}

// Setup resolves a fixture by calling build, registers its teardown, and returns the
// value. The function returns no error: a fixture that cannot be constructed reports it
// through the test's own failure mechanism (t.Fatal and friends), which keeps the
// construction path free of a second, parallel error channel.
func Setup[T any](stack *FixtureStack, build func() (T, TeardownFunc)) T {
	value, hook := build()
	stack.Push(hook)
	return value
}
