package io.github.archont561.playtest;

import java.util.List;
import java.util.Objects;

/**
 * What happened while releasing a test's fixtures.
 *
 * <p>The report is not a boolean. A consumer has to be able to tell <em>which</em> cleanup
 * failed, because hiding it is what the parity contract forbids; reducing it to a pass or
 * fail would discard the only actionable part.
 */
public final class TeardownReport {

    private final int ran;
    private final List<String> failures;

    public TeardownReport(int ran, List<String> failures) {
        this.ran = ran;
        this.failures = List.copyOf(Objects.requireNonNull(failures, "failures"));
    }

    /** How many hooks were called. */
    public int ran() {
        return ran;
    }

    /** One message per hook that failed, in the order the hooks ran. */
    public List<String> failures() {
        return failures;
    }

    /** Whether every hook completed without throwing. */
    public boolean isClean() {
        return failures.isEmpty();
    }

    /**
     * Describe every failure as one error, or return {@code null} when the report is clean.
     *
     * <p>Returning null rather than throwing keeps the decision with the caller: a test
     * harness decides how a dirty teardown is reported, and the engine does not invent a
     * failure channel of its own.
     */
    public AssertionError toError() {
        if (isClean()) {
            return null;
        }
        return new AssertionError(
                "playtest: " + failures.size() + " teardown failure(s): " + failures);
    }
}
