package io.github.archont561.playtest;

import java.util.ArrayList;
import java.util.List;

/**
 * Cross-language conformance suites for the fixture contract (Spec 0005 §1).
 *
 * <p>Each method is one cell of the {@code (capability x target)} matrix in
 * {@code backlog/conformance.yml}. The capabilities exercised here are the ones this target
 * claims; the remaining ones are declared in the manifest but not claimed, so they report
 * as {@code not_implemented} rather than as passing.
 *
 * <p>The assertions use observable behaviour through the public API only. The module is
 * compiled with plain {@code javac} and has no test framework, so failures are reported by
 * throwing {@link AssertionError}, which the CLI turns into a non-zero exit status.
 */
public final class ConformanceTest {

    private ConformanceTest() {}

    /** Run every capability check, stopping at the first failure. */
    public static void main(String[] args) {
        fixtureExplicitResolution();
        teardownLifo();
        teardownFailureSafety();
        bodyFailureStillTearsDown();
        skipOutcomeIdentified();
        System.out.println("Java conformance suite passed");
    }

    /** {@code fixture_explicit_resolution}: only requested fixtures are constructed. */
    private static void fixtureExplicitResolution() {
        List<String> trace = new ArrayList<>();
        FixtureStack stack = new FixtureStack();

        // A fixture that exists but is never passed to setup, so nothing about it runs.
        FixtureStack.Fixture<String> unused = () -> {
            trace.add("unused:setup");
            return new FixtureStack.Resolved<>("never", () -> trace.add("unused:teardown"));
        };
        check(unused != null, "the unused fixture is declared but never resolved");

        try {
            stack.setup(() -> {
                trace.add("requested:setup");
                return new FixtureStack.Resolved<>("value", () -> trace.add("requested:teardown"));
            });
        } catch (Exception err) {
            throw new AssertionError("setup must not fail here", err);
        }

        check(trace.equals(List.of("requested:setup")), "construction trace = " + trace);
        check(stack.size() == 1, "only the requested fixture is held, got " + stack.size());
    }

    /** {@code teardown_lifo}: cleanup is the exact reverse of successful creation. */
    private static void teardownLifo() {
        List<String> trace = new ArrayList<>();
        FixtureStack stack = new FixtureStack();
        for (String name : List.of("a", "b", "c")) {
            try {
                stack.setup(() -> {
                    trace.add(name + ":setup");
                    return new FixtureStack.Resolved<>(name, () -> trace.add(name + ":teardown"));
                });
            } catch (Exception err) {
                throw new AssertionError("setup must not fail here", err);
            }
        }

        TeardownReport report = stack.teardownAll();

        check(report.ran() == 3, "ran " + report.ran() + " hooks, want 3");
        check(report.isClean(), "teardown reported failures: " + report.failures());
        List<String> want =
                List.of(
                        "a:setup",
                        "b:setup",
                        "c:setup",
                        "c:teardown",
                        "b:teardown",
                        "a:teardown");
        check(trace.equals(want), "teardown must be the exact reverse of setup, got " + trace);
    }

    /** {@code teardown_failure_safety}: one failing cleanup never stops the rest. */
    private static void teardownFailureSafety() {
        List<String> trace = new ArrayList<>();
        FixtureStack stack = new FixtureStack();
        try {
            stack.setup(() -> {
                trace.add("outer:setup");
                return new FixtureStack.Resolved<>("outer", () -> trace.add("outer:teardown"));
            });
        } catch (Exception err) {
            throw new AssertionError("setup must not fail here", err);
        }
        stack.push(
                () -> {
                    trace.add("boom:teardown");
                    throw new IllegalStateException("deliberate conformance failure");
                });

        TeardownReport report = stack.teardownAll();

        check(!report.isClean(), "the failing cleanup must be reported");
        check(report.failures().size() == 1, "recorded " + report.failures().size() + " failures");
        check(trace.contains("boom:teardown"), "the failing fixture's cleanup ran");
        check(
                trace.contains("outer:teardown"),
                "the fixture created before the failure must still be released");
        AssertionError error = report.toError();
        check(
                error != null && error.getMessage().contains("deliberate"),
                "the original cause survives into the error");
    }

    /** The harness must surface a body failure and still release every fixture. */
    private static void bodyFailureStillTearsDown() {
        List<String> trace = new ArrayList<>();
        FixtureStack stack = new FixtureStack();
        try {
            stack.setup(() -> {
                trace.add("held:setup");
                return new FixtureStack.Resolved<>("held", () -> trace.add("held:teardown"));
            });
        } catch (Exception err) {
            throw new AssertionError("setup must not fail here", err);
        }

        boolean threw = false;
        try {
            stack.run(
                    () -> {
                        throw new IllegalStateException("body failure");
                    });
        } catch (IllegalStateException err) {
            threw = true;
        } catch (Exception err) {
            throw new AssertionError("the body's own failure must be surfaced", err);
        }

        check(threw, "the body's failure is surfaced");
        check(stack.isEmpty(), "every fixture was released, " + stack.size() + " left");
        check(trace.contains("held:teardown"), "the fixture was torn down despite the failure");
    }

    /** {@code skip_outcome_identified}: a skip carries an explicit outcome and a reason. */
    private static void skipOutcomeIdentified() {
        Skip skip = Skip.skip("gpu rendering", "requires a GPU");

        check(
                "SKIPPED gpu rendering: requires a GPU".equals(skip.reportLine()),
                "report line = " + skip.reportLine());
        check("gpu rendering".equals(skip.test()), "the skipped test is named");
        check("requires a GPU".equals(skip.reason()), "the reason survives into the record");
    }

    /** Fail with a message the CLI can report. */
    private static void check(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }
}
