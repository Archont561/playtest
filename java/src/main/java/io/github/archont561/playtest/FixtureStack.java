package io.github.archont561.playtest;

import java.util.ArrayList;
import java.util.List;

/**
 * Dependency-injected fixtures with strict LIFO teardown.
 *
 * <p>A fixture is a value a test asks for; the engine constructs it, hands it over, and
 * records how to release it. Cleanup runs in the exact reverse of construction order, so a
 * dependency can never observe a resource that depended on it being released first.
 *
 * <p>Teardown is failure-safe: one cleanup that throws does not prevent the others, and
 * the failures are reported without replacing the test's own outcome.
 *
 * <p>The class is deliberately framework-free. This module is compiled with plain
 * {@code javac} and its tests are plain {@code main} methods, so the engine cannot depend
 * on JUnit; JUnit users wrap it in a {@code TestWatcher}, which is a few lines.
 */
public final class FixtureStack {

    /** Releases one fixture. */
    @FunctionalInterface
    public interface Teardown {
        void close() throws Exception;
    }

    /** Constructs a fixture and returns its value together with its cleanup. */
    @FunctionalInterface
    public interface Fixture<T> {
        Resolved<T> create() throws Exception;
    }

    /** A fixture's value and the cleanup that releases it. */
    public static final class Resolved<T> {
        private final T value;
        private final Teardown teardown;

        public Resolved(T value, Teardown teardown) {
            this.value = value;
            this.teardown = teardown;
        }

        public T value() {
            return value;
        }

        public Teardown teardown() {
            return teardown;
        }
    }

    private final List<Runnable> hooks = new ArrayList<>();

    /** How many fixtures are still awaiting teardown. */
    public int size() {
        return hooks.size();
    }

    /** Whether every fixture has been released. */
    public boolean isEmpty() {
        return hooks.isEmpty();
    }

    /**
     * Register a teardown hook.
     *
     * <p>Push order must match construction order: register the hook immediately after the
     * fixture exists, so a failed construction leaves nothing to release.
     */
    public void push(Teardown teardown) {
        if (teardown == null) {
            return;
        }
        hooks.add(() -> {
            try {
                teardown.close();
            } catch (Throwable err) {
                // Throwable, not Exception: an Error from a cleanup (an AssertionError, say)
                // must not abort the remaining hooks. Failure safety covers it too.
                throw new TeardownFailure(err);
            }
        });
    }

    /**
     * Construct a fixture, register its cleanup, and return its value.
     *
     * <p>Order is the guarantee: the value is only returned once its cleanup is on the
     * stack, so a caller cannot hold a resource whose release was never scheduled.
     */
    public <T> T setup(Fixture<T> fixture) throws Exception {
        Resolved<T> resolved = fixture.create();
        push(resolved.teardown());
        return resolved.value();
    }

    /**
     * Run every hook in reverse registration order and report what happened.
     *
     * <p>A hook that throws is recorded and the remaining hooks still run. The stack is
     * empty afterwards, so a second call is a no-op rather than a double release.
     */
    public TeardownReport teardownAll() {
        List<String> failures = new ArrayList<>();
        int ran = 0;
        while (!hooks.isEmpty()) {
            Runnable hook = hooks.remove(hooks.size() - 1);
            ran++;
            try {
                hook.run();
            } catch (TeardownFailure failure) {
                failures.add(describe(failure.getCause()));
            } catch (RuntimeException err) {
                failures.add(describe(err));
            }
        }
        return new TeardownReport(ran, failures);
    }

    /** Render a cleanup failure so the message survives into the report. */
    private static String describe(Throwable err) {
        if (err == null) {
            return "fixture teardown failed without a cause";
        }
        String message = err.getMessage();
        return message == null || message.isBlank() ? err.getClass().getName() : message;
    }

    /** Wraps a checked failure so {@link Teardown} can stay a functional interface. */
    private static final class TeardownFailure extends RuntimeException {
        private static final long serialVersionUID = 1L;

        TeardownFailure(Throwable cause) {
            super(cause);
        }
    }

    /**
     * Run {@code body} and then release every fixture.
     *
     * <p>Teardown runs whether the body returns or throws. When the body throws, teardown
     * completes first and the original error is rethrown, so a cleanup problem never
     * becomes the headline reason a test failed.
     */
    public TeardownReport run(CheckedRunnable body) throws Exception {
        Throwable bodyFailure = null;
        try {
            body.run();
        } catch (Exception | Error err) {
            bodyFailure = err;
        }
        TeardownReport report = teardownAll();
        if (bodyFailure instanceof Exception) {
            throw (Exception) bodyFailure;
        }
        if (bodyFailure instanceof Error) {
            throw (Error) bodyFailure;
        }
        return report;
    }

    /** A body that may throw, so {@link #run} needs no lambda gymnastics. */
    @FunctionalInterface
    public interface CheckedRunnable {
        void run() throws Exception;
    }
}
