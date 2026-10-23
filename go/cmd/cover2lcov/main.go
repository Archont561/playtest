package main

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

func main() {
	inputFile := "coverage.out"
	outputFile := "coverage/lcov.info"
	if len(os.Args) > 1 {
		inputFile = os.Args[1]
	}
	if len(os.Args) > 2 {
		outputFile = os.Args[2]
	}

	file, err := os.Open(inputFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error opening %s: %v\n", inputFile, err)
		os.Exit(1)
	}
	defer func() {
		_ = file.Close()
	}()

	if err := os.MkdirAll(filepath.Dir(outputFile), 0755); err != nil {
		fmt.Fprintf(os.Stderr, "Error creating dir: %v\n", err)
		os.Exit(1)
	}

	out, err := os.Create(outputFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error creating %s: %v\n", outputFile, err)
		os.Exit(1)
	}
	defer func() {
		_ = out.Close()
	}()

	scanner := bufio.NewScanner(file)
	type fileRecord struct {
		lines map[int]int
	}
	records := make(map[string]*fileRecord)
	var fileOrder []string

	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "mode:") {
			continue
		}
		parts := strings.Split(line, ":")
		if len(parts) < 2 {
			continue
		}
		srcFile := parts[0]
		// Normalize file path if package prefix exists
		if idx := strings.Index(srcFile, "playtest/go/"); idx != -1 {
			srcFile = srcFile[idx+len("playtest/go/"):]
		}

		rest := strings.Split(parts[1], " ")
		if len(rest) < 3 {
			continue
		}
		rangeParts := strings.Split(rest[0], ",")
		count, _ := strconv.Atoi(rest[2])

		startLine, _ := strconv.Atoi(strings.Split(rangeParts[0], ".")[0])
		endLine, _ := strconv.Atoi(strings.Split(rangeParts[1], ".")[0])

		rec, exists := records[srcFile]
		if !exists {
			rec = &fileRecord{lines: make(map[int]int)}
			records[srcFile] = rec
			fileOrder = append(fileOrder, srcFile)
		}

		for l := startLine; l <= endLine; l++ {
			rec.lines[l] += count
		}
	}

	for _, src := range fileOrder {
		rec := records[src]
		_, _ = fmt.Fprintf(out, "SF:%s\n", src)
		lf := len(rec.lines)
		lh := 0
		for l := 1; l <= 10000; l++ {
			if c, ok := rec.lines[l]; ok {
				_, _ = fmt.Fprintf(out, "DA:%d,%d\n", l, c)
				if c > 0 {
					lh++
				}
			}
		}
		_, _ = fmt.Fprintf(out, "LF:%d\n", lf)
		_, _ = fmt.Fprintf(out, "LH:%d\n", lh)
		_, _ = fmt.Fprintln(out, "end_of_record")
	}

	fmt.Printf("Go cover2lcov: %s -> %s\n", inputFile, outputFile)
}
