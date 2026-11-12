// Package clean shows the shapes PCC-S01 and PCC-S07 accept in Java.
package clean;

import java.util.logging.Logger;

final class Service {
    private static final Logger LOG = Logger.getLogger(Service.class.getName());

    void start(int port) {
        LOG.info("starting " + port);
    }

    void stop(Exception error) {
        try {
            close();
        } catch (RuntimeException failure) {
            LOG.severe("close failed: " + failure.getMessage());
        }
    }

    private void close() {
        throw new IllegalStateException("already closed");
    }
}
