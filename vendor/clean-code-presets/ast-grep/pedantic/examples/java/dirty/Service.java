// Package dirty shows the shapes PCC-S01 and PCC-S07 reject in Java.
package dirty;

final class Service {
    void start(int port) {
        System.out.println("starting"); // expect: pcc-s01-print-java
        System.err.println("port " + port); // expect: pcc-s01-print-java
    }

    void stop(Exception error) {
        try { // expect: pcc-s07-empty-catch-java
            close();
        } catch (RuntimeException failure) {
        }
        error.printStackTrace(); // expect: pcc-s01-print-java
    }

    private void close() {
        throw new IllegalStateException("already closed");
    }
}
