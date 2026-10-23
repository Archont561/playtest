package io.github.archont561.playtest;

public class PlaytestTest {
    public static void main(String[] args) {
        if (!"0.1.0".equals(Playtest.VERSION)) {
            throw new AssertionError("Expected 0.1.0, got " + Playtest.VERSION);
        }
        System.out.println("Java PlaytestTest passed: " + Playtest.VERSION);
    }
}
