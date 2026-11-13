package playtest

import "fmt"

// Skip records that a test did not run, and why (Spec 0005 §1.4).
//
// A skipped test must be distinguishable from a passing one. Reporting a skip as a pass
// is a false green, so this type carries an explicit outcome instead of returning a bool
// or silence: the caller either reports a [Skip] or does not, and either way the reason
// travels with the record.
//
// The record is separate from the runner on purpose. `t.Skip` reports a skip to `go test`
// but says nothing to Playtest, so [SkipNow] does both: record here, delegate there. The
// record is the part that can be asserted on, which is what makes the capability provable
// rather than merely claimed.
type Skip struct {
	// Test is the declared name of the test that did not run.
	Test string
	// Reason is why it did not run. A skip without a reason is not actionable.
	Reason string
}

// RecordSkip records a skip and returns it so the caller can report it.
func RecordSkip(testName, reason string) Skip {
	return Skip{Test: testName, Reason: reason}
}

// Skipped reports whether this record describes a skip. It is always true, and exists so a
// caller holding an optional record can ask without special-casing a nil pointer.
func (s Skip) Skipped() bool {
	return true
}

// ReportLine is the single line a report parser recognizes.
func (s Skip) ReportLine() string {
	return fmt.Sprintf("SKIPPED %s: %s", s.Test, s.Reason)
}

// SkipNow records the skip and then delegates to the runner.
//
// Order matters: the record is created before `t.Skip` unwinds the goroutine, so a caller
// that inspects it afterwards still sees it. `testing.T.Skip` is the idiomatic Go skip and
// stays in charge of how the skip is reported to `go test`.
func SkipNow(t Skipper, testName, reason string) Skip {
	record := RecordSkip(testName, reason)
	t.Skip(record.ReportLine())
	return record
}

// Skipper is the part of *testing.T that [SkipNow] needs.
//
// It is an interface rather than a concrete *testing.T so the skip path stays usable
// outside the go test binary, and so a conformance suite can assert the delegation without
// ending the test that asserts it.
type Skipper interface {
	// Skip ends the current test and marks it as skipped.
	Skip(args ...any)
}

// SkipIf records a conditional skip and reports whether the test should stop.
//
// The condition is evaluated when the test runs, not when it is declared, so a skip can
// depend on the host or on a fixture. A false condition records nothing and returns false,
// which lets the caller continue:
//
//	if playtest.SkipIf(t, !haveGPU(), "needs a GPU", "rendering requires hardware") {
//		return
//	}
func SkipIf(t Skipper, condition bool, testName, reason string) bool {
	if !condition {
		return false
	}
	SkipNow(t, testName, reason)
	return true
}
