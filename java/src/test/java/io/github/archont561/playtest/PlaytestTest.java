package io.github.archont561.playtest;

public class PlaytestTest {
    public static void main(String[] args) {
        if (!Playtest.VERSION.matches("\\d+\\.\\d+\\.\\d+(?:-[A-Za-z0-9.-]+)?")) {
            throw new AssertionError("Expected a semantic version, got " + Playtest.VERSION);
        }
        System.out.println("Java PlaytestTest passed: " + Playtest.VERSION);
    }
}
