//go:build !windows

package main

import "syscall"

func execBinary(binPath string, args []string, env []string) (int, error) {
	argv := make([]string, 0, len(args)+1)
	argv = append(argv, binPath)
	argv = append(argv, args...)
	err := syscall.Exec(binPath, argv, env)
	return ExitEnvironment, err
}
