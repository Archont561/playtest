package dirty;

final class Variants {
    void report(Exception error) {
        System.out.print("starting"); // expect: pcc-s01-print-java
        System.err.print("failed"); // expect: pcc-s01-print-java
        error.printStackTrace(); // expect: pcc-s01-print-java
    }

    void close() {
        try { // expect: pcc-s07-empty-catch-java
            throw new IllegalStateException();
        } catch (IllegalStateException failure) {}
    }
}
