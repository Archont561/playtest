package main

import (
	"context"
	"fmt"
	"io"
	"os"
)

func run(args []string, stderr io.Writer, runner func(string, []string, []string) (int, error)) int {
	cfg, err := DefaultConfig()
	if err != nil {
		_, _ = fmt.Fprintln(stderr, err.Error())
		return ExitEnvironment
	}
	binPath, err := EnsureBinary(context.Background(), cfg)
	if err != nil {
		_, _ = fmt.Fprintln(stderr, err.Error())
		return ExitEnvironment
	}
	code, execErr := runner(binPath, args, os.Environ())
	if execErr != nil {
		_, _ = fmt.Fprintf(
			stderr,
			"error: failed to execute Playtest CLI binary at %s: %v\n  fix: delete %s and re-run to refresh the cached binary\n",
			binPath,
			execErr,
			binPath,
		)
		return ExitEnvironment
	}
	return code
}

func main() {
	os.Exit(run(os.Args[1:], os.Stderr, execBinary))
}
