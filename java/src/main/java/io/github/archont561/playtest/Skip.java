package io.github.archont561.playtest;

import java.util.Objects;

/**
 * A skipped test and the reason it did not run.
 *
 * <p>A skipped test must be distinguishable from a passing one; reporting a skip as a pass
 * is a false green, the worst outcome for a test framework. The record carries the reason
 * because a skip nobody can explain is not actionable.
 *
 * <p>Java has no built-in skip signal outside a framework. {@link #reportLine()} is the
 * single line a reporter recognizes, and {@link #skip(String, String)} is the convenience
 * factory; a JUnit user converts the record into {@code Assumptions.assumeTrue(false)},
 * which is a few lines and keeps JUnit in charge of how the skip is reported.
 */
public final class Skip {

    private final String test;
    private final String reason;

    public Skip(String test, String reason) {
        this.test = Objects.requireNonNull(test, "a skip must name the test");
        this.reason = Objects.requireNonNull(reason, "a skip must carry a reason");
    }

    /** Build a skip record. */
    public static Skip skip(String test, String reason) {
        return new Skip(test, reason);
    }

    /** The test that did not run. */
    public String test() {
        return test;
    }

    /** Why it did not run. */
    public String reason() {
        return reason;
    }

    /** The single line a report parser recognizes. */
    public String reportLine() {
        return "SKIPPED " + test + ": " + reason;
    }
}
