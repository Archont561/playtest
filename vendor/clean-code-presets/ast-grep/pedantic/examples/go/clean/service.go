// Package clean shows the shape PCC-S01 accepts in Go.
package clean

import "log/slog"

// Start reports a service start through structured logging.
func Start(port int) {
	slog.Info("starting", "port", port)
}
