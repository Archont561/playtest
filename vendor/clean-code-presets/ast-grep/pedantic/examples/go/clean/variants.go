package clean

import (
	"fmt"
	"io"
)

// Report writes to a caller-provided writer rather than standard output.
func Report(w io.Writer, port int) error {
	_, err := fmt.Fprintf(w, "port=%d\n", port)
	return err
}

// Format returns a message without printing it.
func Format(port int) string {
	return fmt.Sprintf("port=%d", port)
}
