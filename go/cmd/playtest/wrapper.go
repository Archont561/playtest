// Command playtest downloads, verifies, caches, and executes the Playtest CLI binary.
package main

import (
	"archive/tar"
	"archive/zip"
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	playtest "github.com/Archont561/playtest/go"
)

const (
	// ExitEnvironment matches Spec 0008 §7 exit status 3 for environment/setup failures.
	ExitEnvironment = 3

	// DefaultReleaseBaseURL is the canonical GitHub Releases download prefix.
	DefaultReleaseBaseURL = "https://github.com/Archont561/playtest/releases/download"

	maxDownloadBytes = 256 << 20
)

// TargetSpec describes the release archive for a supported OS and CPU pair.
type TargetSpec struct {
	Triple     string
	Ext        string
	BinaryName string
}

// ArchiveFilename returns the GitHub Release asset name for the target.
func (t TargetSpec) ArchiveFilename() string {
	return "playtest-cli-" + t.Triple + t.Ext
}

// WrapperError represents an actionable setup failure that exits with status 3.
type WrapperError struct {
	Summary string
	URL     string
	Fix     string
}

func (e *WrapperError) Error() string {
	return fmt.Sprintf("error: %s\n  url: %s\n  fix: %s", e.Summary, e.URL, e.Fix)
}

// Config controls platform resolution, release URLs, and per-user cache location.
type Config struct {
	Version        string
	GOOS           string
	GOARCH         string
	ReleaseBaseURL string
	CacheRoot      string
	HTTPClient     *http.Client
}

// DefaultConfig builds a Config from the host runtime and optional environment overrides.
func DefaultConfig() (Config, error) {
	baseURL := strings.TrimRight(os.Getenv("PLAYTEST_RELEASE_BASE_URL"), "/")
	if baseURL == "" {
		baseURL = DefaultReleaseBaseURL
	}
	cacheRoot := os.Getenv("PLAYTEST_CACHE_DIR")
	if cacheRoot == "" {
		userCache, err := os.UserCacheDir()
		if err != nil {
			return Config{}, &WrapperError{
				Summary: fmt.Sprintf("could not determine per-user cache directory: %v", err),
				URL:     fmt.Sprintf("%s/v%s/sha256.sum", baseURL, playtest.Version),
				Fix:     "set PLAYTEST_CACHE_DIR or HOME/LOCALAPPDATA to a writable user directory",
			}
		}
		cacheRoot = userCache
	}
	return Config{
		Version:        playtest.Version,
		GOOS:           runtime.GOOS,
		GOARCH:         runtime.GOARCH,
		ReleaseBaseURL: baseURL,
		CacheRoot:      cacheRoot,
		HTTPClient:     &http.Client{Timeout: 120 * time.Second},
	}, nil
}

// ResolveTarget maps GOOS and GOARCH to a supported Spec 0012 §3 target.
func ResolveTarget(goos, goarch, version, baseURL string) (TargetSpec, error) {
	switch goos + "/" + goarch {
	case "linux/amd64":
		return TargetSpec{Triple: "x86_64-unknown-linux-musl", Ext: ".tar.gz", BinaryName: "playtest"}, nil
	case "linux/arm64":
		return TargetSpec{Triple: "aarch64-unknown-linux-musl", Ext: ".tar.gz", BinaryName: "playtest"}, nil
	case "darwin/amd64":
		return TargetSpec{Triple: "x86_64-apple-darwin", Ext: ".tar.gz", BinaryName: "playtest"}, nil
	case "darwin/arm64":
		return TargetSpec{Triple: "aarch64-apple-darwin", Ext: ".tar.gz", BinaryName: "playtest"}, nil
	case "windows/amd64":
		return TargetSpec{Triple: "x86_64-pc-windows-msvc", Ext: ".zip", BinaryName: "playtest.exe"}, nil
	default:
		return TargetSpec{}, &WrapperError{
			Summary: fmt.Sprintf("unsupported platform %s/%s for prebuilt Playtest CLI v%s", goos, goarch, version),
			URL:     fmt.Sprintf("%s/v%s/sha256.sum", strings.TrimRight(baseURL, "/"), version),
			Fix:     fmt.Sprintf("build from source with Rust >= 1.88: cargo install playtest-cli --version %s --locked", version),
		}
	}
}

// EnsureBinary returns a verified cached CLI binary path, downloading on first use.
func EnsureBinary(ctx context.Context, cfg Config) (string, error) {
	baseURL := strings.TrimRight(cfg.ReleaseBaseURL, "/")
	if baseURL == "" {
		baseURL = DefaultReleaseBaseURL
	}
	spec, err := ResolveTarget(cfg.GOOS, cfg.GOARCH, cfg.Version, baseURL)
	if err != nil {
		return "", err
	}
	binPath := filepath.Join(cfg.CacheRoot, "playtest", "cli", "v"+cfg.Version, spec.Triple, spec.BinaryName)
	if info, statErr := os.Stat(binPath); statErr == nil && info.Mode().IsRegular() {
		return binPath, nil
	}

	client := cfg.HTTPClient
	if client == nil {
		client = &http.Client{Timeout: 120 * time.Second}
	}

	sumURL := fmt.Sprintf("%s/v%s/sha256.sum", baseURL, cfg.Version)
	sumBytes, err := fetchReleaseAsset(ctx, client, sumURL, cfg.Version)
	if err != nil {
		return "", err
	}
	expectedHex, err := lookupChecksum(string(sumBytes), spec.ArchiveFilename(), sumURL, cfg.Version)
	if err != nil {
		return "", err
	}

	archiveURL := fmt.Sprintf("%s/v%s/%s", baseURL, cfg.Version, spec.ArchiveFilename())
	archiveBytes, err := fetchReleaseAsset(ctx, client, archiveURL, cfg.Version)
	if err != nil {
		return "", err
	}

	digest := sha256.Sum256(archiveBytes)
	actualHex := hex.EncodeToString(digest[:])
	if !strings.EqualFold(actualHex, expectedHex) {
		return "", &WrapperError{
			Summary: fmt.Sprintf("SHA-256 checksum mismatch for %s (expected %s, got %s)", spec.ArchiveFilename(), expectedHex, actualHex),
			URL:     archiveURL,
			Fix:     fmt.Sprintf("re-run over a clean network connection or verify GitHub Release v%s assets", cfg.Version),
		}
	}

	binaryPayload, err := extractBinaryBytes(archiveBytes, spec, archiveURL)
	if err != nil {
		return "", err
	}
	if err := writeAtomically(binPath, binaryPayload); err != nil {
		return "", &WrapperError{
			Summary: fmt.Sprintf("failed to store verified CLI binary in cache (%s): %v", binPath, err),
			URL:     archiveURL,
			Fix:     "ensure your user cache directory is writable and has free disk space",
		}
	}
	return binPath, nil
}

func fetchReleaseAsset(ctx context.Context, client *http.Client, url, version string) ([]byte, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("invalid release asset URL: %v", err),
			URL:     url,
			Fix:     "check PLAYTEST_RELEASE_BASE_URL or use the default GitHub Releases endpoint",
		}
	}
	req.Header.Set("User-Agent", "playtest-go-wrapper/"+version)
	resp, err := client.Do(req)
	if err != nil {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("network failure downloading Playtest CLI v%s asset: %v", version, err),
			URL:     url,
			Fix:     "check network connectivity to github.com and retry; once cached, later runs need no network",
		}
	}
	defer func() {
		_ = resp.Body.Close()
	}()
	if resp.StatusCode == http.StatusNotFound {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("GitHub Release asset for Playtest CLI v%s was not found (HTTP 404)", version),
			URL:     url,
			Fix:     fmt.Sprintf("verify GitHub Release v%s is published; the wrapper never falls back to another version", version),
		}
	}
	if resp.StatusCode != http.StatusOK {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("unexpected HTTP status %d while downloading Playtest CLI v%s", resp.StatusCode, version),
			URL:     url,
			Fix:     "retry once GitHub Releases is reachable or install from another Spec 0012 channel",
		}
	}
	body, readErr := io.ReadAll(io.LimitReader(resp.Body, maxDownloadBytes))
	if readErr != nil {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("interrupted download for Playtest CLI v%s: %v", version, readErr),
			URL:     url,
			Fix:     "retry the command over a stable network connection",
		}
	}
	return body, nil
}

func lookupChecksum(manifest, filename, sumURL, version string) (string, error) {
	for _, rawLine := range strings.Split(manifest, "\n") {
		fields := strings.Fields(strings.TrimSpace(rawLine))
		if len(fields) < 2 {
			continue
		}
		candidate := strings.TrimPrefix(fields[1], "*")
		if candidate == filename || filepath.Base(candidate) == filename {
			if len(fields[0]) == 64 {
				return strings.ToLower(fields[0]), nil
			}
		}
	}
	return "", &WrapperError{
		Summary: fmt.Sprintf("checksum entry for %s is missing from sha256.sum in release v%s", filename, version),
		URL:     sumURL,
		Fix:     fmt.Sprintf("verify that GitHub Release v%s attached all platform archives and sha256.sum", version),
	}
}

func extractBinaryBytes(archiveBytes []byte, spec TargetSpec, archiveURL string) ([]byte, error) {
	if spec.Ext == ".zip" {
		return extractFromZip(archiveBytes, spec.BinaryName, archiveURL)
	}
	return extractFromTarGz(archiveBytes, spec.BinaryName, archiveURL)
}

func extractFromTarGz(archiveBytes []byte, binaryName, archiveURL string) ([]byte, error) {
	gz, err := gzip.NewReader(bytes.NewReader(archiveBytes))
	if err != nil {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("invalid gzip archive: %v", err),
			URL:     archiveURL,
			Fix:     "inspect the release archive on GitHub Releases and retry",
		}
	}
	defer func() {
		_ = gz.Close()
	}()
	tr := tar.NewReader(gz)
	for {
		hdr, nextErr := tr.Next()
		if errors.Is(nextErr, io.EOF) {
			break
		}
		if nextErr != nil {
			return nil, &WrapperError{
				Summary: fmt.Sprintf("corrupt tar stream: %v", nextErr),
				URL:     archiveURL,
				Fix:     "inspect the release archive on GitHub Releases and retry",
			}
		}
		if hdr.Typeflag == tar.TypeReg && filepath.Base(hdr.Name) == binaryName {
			return io.ReadAll(io.LimitReader(tr, maxDownloadBytes))
		}
	}
	return nil, &WrapperError{
		Summary: fmt.Sprintf("executable %s was not found inside release archive", binaryName),
		URL:     archiveURL,
		Fix:     "verify the GitHub Release archive layout matches Spec 0012 §3",
	}
}

func extractFromZip(archiveBytes []byte, binaryName, archiveURL string) ([]byte, error) {
	zr, err := zip.NewReader(bytes.NewReader(archiveBytes), int64(len(archiveBytes)))
	if err != nil {
		return nil, &WrapperError{
			Summary: fmt.Sprintf("invalid zip archive: %v", err),
			URL:     archiveURL,
			Fix:     "inspect the release archive on GitHub Releases and retry",
		}
	}
	for _, member := range zr.File {
		if !member.FileInfo().IsDir() && filepath.Base(member.Name) == binaryName {
			rc, openErr := member.Open()
			if openErr != nil {
				return nil, openErr
			}
			data, readErr := io.ReadAll(io.LimitReader(rc, maxDownloadBytes))
			_ = rc.Close()
			return data, readErr
		}
	}
	return nil, &WrapperError{
		Summary: fmt.Sprintf("executable %s was not found inside release zip archive", binaryName),
		URL:     archiveURL,
		Fix:     "verify the GitHub Release archive layout matches Spec 0012 §3",
	}
}

func writeAtomically(dest string, payload []byte) error {
	dir := filepath.Dir(dest)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(dir, ".playtest-tmp-*")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	cleanup := true
	defer func() {
		if cleanup {
			_ = os.Remove(tmpName)
		}
	}()
	if _, err := tmp.Write(payload); err != nil {
		_ = tmp.Close()
		return err
	}
	if err := tmp.Chmod(0o755); err != nil {
		_ = tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	if err := os.Rename(tmpName, dest); err != nil {
		return err
	}
	cleanup = false
	return nil
}
