package io.github.archont561.playtest;

import java.io.IOException;
import java.io.InputStream;
import java.util.Properties;

/**
 * Playtest for Java: dependency-injected fixtures with strict LIFO teardown,
 * property-based testing, and code-first BDD sharing one contract across nine
 * language targets.
 */
public final class Playtest {
    public static final String VERSION = loadVersion();

    private Playtest() {}

    private static String loadVersion() {
        try (InputStream stream = Playtest.class.getResourceAsStream("version.properties")) {
            if (stream == null) {
                throw new IllegalStateException("Missing playtest version.properties resource");
            }
            Properties props = new Properties();
            props.load(stream);
            String resolved = props.getProperty("version");
            if (resolved == null || resolved.isBlank() || resolved.contains("${")) {
                throw new IllegalStateException("Unfiltered playtest version property: " + resolved);
            }
            return resolved.trim();
        } catch (IOException err) {
            throw new IllegalStateException("Failed to read playtest version.properties", err);
        }
    }
}
