package main

import (
	"archive/tar"
	"archive/zip"
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync/atomic"
	"testing"
)

func buildTestTarGz(t *testing.T, stem, binName string, payload []byte) []byte {
	t.Helper()
	var buf bytes.Buffer
	gz := gzip.NewWriter(&buf)
	tw := tar.NewWriter(gz)
	hdr := &tar.Header{
		Name:     stem + "/" + binName,
		Mode:     0o755,
		Size:     int64(len(payload)),
		Typeflag: tar.TypeReg,
	}
	if err := tw.WriteHeader(hdr); err != nil {
		t.Fatalf("tar header: %v", err)
	}
	if _, err := tw.Write(payload); err != nil {
		t.Fatalf("tar write: %v", err)
	}
	if err := tw.Close(); err != nil {
		t.Fatalf("tar close: %v", err)
	}
	if err := gz.Close(); err != nil {
		t.Fatalf("gzip close: %v", err)
	}
	return buf.Bytes()
}

func buildTestZip(t *testing.T, stem, binName string, payload []byte) []byte {
	t.Helper()
	var buf bytes.Buffer
	zw := zip.NewWriter(&buf)
	w, err := zw.Create(stem + "/" + binName)
	if err != nil {
		t.Fatalf("zip create: %v", err)
	}
	if _, err := w.Write(payload); err != nil {
		t.Fatalf("zip write: %v", err)
	}
	if err := zw.Close(); err != nil {
		t.Fatalf("zip close: %v", err)
	}
	return buf.Bytes()
}

func sha256Line(data []byte, filename string) string {
	sum := sha256.Sum256(data)
	return fmt.Sprintf("%s  %s\n", hex.EncodeToString(sum[:]), filename)
}

func TestEnsureBinaryTarGzAndOfflineCacheReuse(t *testing.T) {
	payload := []byte("#!/usr/bin/env sh\necho playtest-go-test\n")
	archive := buildTestTarGz(t, "playtest-cli-x86_64-unknown-linux-musl", "playtest", payload)
	sumBody := sha256Line(archive, "playtest-cli-x86_64-unknown-linux-musl.tar.gz")

	var hits atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		hits.Add(1)
		switch r.URL.Path {
		case "/v0.1.0/sha256.sum":
			_, _ = w.Write([]byte(sumBody))
		case "/v0.1.0/playtest-cli-x86_64-unknown-linux-musl.tar.gz":
			_, _ = w.Write(archive)
		default:
			http.NotFound(w, r)
		}
	}))
	defer srv.Close()

	cacheRoot := t.TempDir()
	cfg := Config{
		Version:        "0.1.0",
		GOOS:           "linux",
		GOARCH:         "amd64",
		ReleaseBaseURL: srv.URL,
		CacheRoot:      cacheRoot,
		HTTPClient:     srv.Client(),
	}
	binPath, err := EnsureBinary(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected EnsureBinary success, got: %v", err)
	}
	gotBytes, readErr := os.ReadFile(binPath)
	if readErr != nil || !bytes.Equal(gotBytes, payload) {
		t.Fatalf("cached binary mismatch: err=%v", readErr)
	}
	if hits.Load() != 2 {
		t.Fatalf("expected 2 HTTP requests on cold cache, got %d", hits.Load())
	}

	// Shut down server completely to prove subsequent calls require zero network access.
	srv.Close()
	cachedAgain, err := EnsureBinary(context.Background(), cfg)
	if err != nil {
		t.Fatalf("expected offline cache hit, got: %v", err)
	}
	if cachedAgain != binPath || hits.Load() != 2 {
		t.Fatalf("expected cached path with zero additional HTTP hits")
	}
}

func TestEnsureBinaryWindowsZipAndAllTargets(t *testing.T) {
	for _, pair := range [][2]string{
		{"linux", "amd64"},
		{"linux", "arm64"},
		{"darwin", "amd64"},
		{"darwin", "arm64"},
		{"windows", "amd64"},
	} {
		spec, err := ResolveTarget(pair[0], pair[1], "0.1.0", DefaultReleaseBaseURL)
		if err != nil || spec.Triple == "" {
			t.Fatalf("ResolveTarget(%v) failed: %v", pair, err)
		}
	}

	payload := []byte("MZ-fake-windows-binary")
	archive := buildTestZip(t, "playtest-cli-x86_64-pc-windows-msvc", "playtest.exe", payload)
	sumBody := sha256Line(archive, "playtest-cli-x86_64-pc-windows-msvc.zip")
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/v0.1.0/sha256.sum":
			_, _ = w.Write([]byte(sumBody))
		case "/v0.1.0/playtest-cli-x86_64-pc-windows-msvc.zip":
			_, _ = w.Write(archive)
		default:
			http.NotFound(w, r)
		}
	}))
	defer srv.Close()

	binPath, err := EnsureBinary(context.Background(), Config{
		Version:        "0.1.0",
		GOOS:           "windows",
		GOARCH:         "amd64",
		ReleaseBaseURL: srv.URL,
		CacheRoot:      t.TempDir(),
		HTTPClient:     srv.Client(),
	})
	if err != nil {
		t.Fatalf("expected Windows zip extraction success: %v", err)
	}
	got, _ := os.ReadFile(binPath)
	if !bytes.Equal(got, payload) {
		t.Fatalf("unexpected Windows binary bytes")
	}
}

func TestErrorCasesMissingReleaseChecksumNetworkUnsupported(t *testing.T) {
	// 1. Unsupported platform
	_, err := EnsureBinary(context.Background(), Config{
		Version:        "0.1.0",
		GOOS:           "freebsd",
		GOARCH:         "riscv64",
		ReleaseBaseURL: DefaultReleaseBaseURL,
		CacheRoot:      t.TempDir(),
	})
	if err == nil || !strings.Contains(err.Error(), "freebsd/riscv64") || !strings.Contains(err.Error(), "fix:") {
		t.Fatalf("expected actionable unsupported platform error, got: %v", err)
	}

	// 2. Missing release (HTTP 404) - must never fall back to another version
	var requestedPaths []string
	srv404 := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		requestedPaths = append(requestedPaths, r.URL.Path)
		http.NotFound(w, r)
	}))
	defer srv404.Close()

	_, err = EnsureBinary(context.Background(), Config{
		Version:        "0.9.9",
		GOOS:           "linux",
		GOARCH:         "amd64",
		ReleaseBaseURL: srv404.URL,
		CacheRoot:      t.TempDir(),
		HTTPClient:     srv404.Client(),
	})
	if err == nil || !strings.Contains(err.Error(), srv404.URL+"/v0.9.9/sha256.sum") || !strings.Contains(err.Error(), "fix:") {
		t.Fatalf("expected missing release error naming URL and fix, got: %v", err)
	}
	if len(requestedPaths) != 1 || requestedPaths[0] != "/v0.9.9/sha256.sum" {
		t.Fatalf("wrapper attempted unexpected fallback URLs: %v", requestedPaths)
	}

	// 3. Checksum mismatch
	goodArchive := buildTestTarGz(t, "playtest-cli-x86_64-unknown-linux-musl", "playtest", []byte("good"))
	tampered := buildTestTarGz(t, "playtest-cli-x86_64-unknown-linux-musl", "playtest", []byte("tampered"))
	sumGood := sha256Line(goodArchive, "playtest-cli-x86_64-unknown-linux-musl.tar.gz")
	srvBadHash := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasSuffix(r.URL.Path, "sha256.sum") {
			_, _ = w.Write([]byte(sumGood))
			return
		}
		_, _ = w.Write(tampered)
	}))
	defer srvBadHash.Close()

	mismatchCache := t.TempDir()
	_, err = EnsureBinary(context.Background(), Config{
		Version:        "0.1.0",
		GOOS:           "linux",
		GOARCH:         "amd64",
		ReleaseBaseURL: srvBadHash.URL,
		CacheRoot:      mismatchCache,
		HTTPClient:     srvBadHash.Client(),
	})
	if err == nil || !strings.Contains(err.Error(), "checksum mismatch") || !strings.Contains(err.Error(), srvBadHash.URL) {
		t.Fatalf("expected checksum mismatch error naming URL, got: %v", err)
	}

	// 4. Network failure
	closedSrv := httptest.NewServer(http.NotFoundHandler())
	closedURL := closedSrv.URL
	closedSrv.Close()
	_, err = EnsureBinary(context.Background(), Config{
		Version:        "0.1.0",
		GOOS:           "linux",
		GOARCH:         "amd64",
		ReleaseBaseURL: closedURL,
		CacheRoot:      t.TempDir(),
	})
	if err == nil || !strings.Contains(err.Error(), closedURL) || !strings.Contains(err.Error(), "fix:") {
		t.Fatalf("expected network failure error naming URL and fix, got: %v", err)
	}
}

func TestRunPassthroughAndFailureExitCode(t *testing.T) {
	payload := []byte("#!/usr/bin/env sh\nexit 0\n")
	spec, err := ResolveTarget(
		func() string {
			cfg, _ := DefaultConfig()
			return cfg.GOOS
		}(),
		func() string {
			cfg, _ := DefaultConfig()
			return cfg.GOARCH
		}(),
		"0.1.0",
		DefaultReleaseBaseURL,
	)
	if err != nil {
		t.Skip("host platform outside matrix")
	}
	var archive []byte
	if spec.Ext == ".zip" {
		archive = buildTestZip(t, "playtest-cli-"+spec.Triple, spec.BinaryName, payload)
	} else {
		archive = buildTestTarGz(t, "playtest-cli-"+spec.Triple, spec.BinaryName, payload)
	}
	sumBody := sha256Line(archive, spec.ArchiveFilename())

	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasSuffix(r.URL.Path, "sha256.sum") {
			_, _ = w.Write([]byte(sumBody))
			return
		}
		_, _ = w.Write(archive)
	}))
	defer srv.Close()

	t.Setenv("PLAYTEST_RELEASE_BASE_URL", srv.URL)
	t.Setenv("PLAYTEST_CACHE_DIR", t.TempDir())

	var stderr bytes.Buffer
	var capturedArgs []string
	code := run([]string{"version", "--verbose"}, &stderr, func(_ string, args []string, _ []string) (int, error) {
		capturedArgs = args
		return 7, nil
	})
	if code != 7 || len(capturedArgs) != 2 || capturedArgs[0] != "version" {
		t.Fatalf("expected exit code 7 and forwarded args, got code=%d args=%v stderr=%s", code, capturedArgs, stderr.String())
	}

	// Failure exit code 3 when download fails
	srv.Close()
	t.Setenv("PLAYTEST_CACHE_DIR", t.TempDir())
	stderr.Reset()
	failCode := run([]string{"version"}, &stderr, func(string, []string, []string) (int, error) {
		return 0, nil
	})
	if failCode != ExitEnvironment || !strings.Contains(stderr.String(), "fix:") {
		t.Fatalf("expected exit status 3 on download failure, got %d (%s)", failCode, stderr.String())
	}
}
