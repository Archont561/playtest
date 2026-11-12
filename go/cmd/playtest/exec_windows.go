//go:build windows

package main

import (
	"errors"
	"os"
	"os/exec"
	"os/signal"
)

func execBinary(binPath string, args []string, env []string) (int, error) {
	cmd := exec.Command(binPath, args...)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Env = env

	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt)
	defer signal.Stop(sigCh)

	if err := cmd.Start(); err != nil {
		return ExitEnvironment, err
	}
	done := make(chan struct{})
	go func() {
		select {
		case sig := <-sigCh:
			if cmd.Process != nil {
				_ = cmd.Process.Signal(sig)
			}
		case <-done:
		}
	}()

	waitErr := cmd.Wait()
	close(done)
	if waitErr == nil {
		return 0, nil
	}
	var exitErr *exec.ExitError
	if errors.As(waitErr, &exitErr) {
		return exitErr.ExitCode(), nil
	}
	return ExitEnvironment, waitErr
}
