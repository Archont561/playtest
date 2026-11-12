package clean;

import java.util.logging.Logger;

final class Variants {
    private static final Logger LOG = Logger.getLogger(Variants.class.getName());

    void close() {
        try {
            throw new IllegalStateException();
        } catch (IllegalStateException failure) {
            LOG.warning(failure.toString());
        }
    }

    String message(int port) {
        return String.format("port=%d", port);
    }
}
