# ADR 0020: Solid Brotli Compression for Embedded SDK Assets

## Status
Accepted (2026-11-11). Amends [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md).

## Context

[ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md) and [Specification 0012 §2](../specs/0012-workspace-crates-and-cli-distribution.md) embed the published files of all eight non-Rust language SDKs inside `playtest-assets` via `build.rs` and `include_dir`, so a single `playtest` CLI binary carries every SDK at one synchronized version.

As Milestones M2 through M5 implement fixture lifecycles, LIFO teardown, code-first BDD, property-based testing, isolation helpers, and JSONL runner reporters across Python, TypeScript, C++, C, C#, Go, Java, and R, the raw embedded SDK trees will grow from the ~25 KB M1 version scaffold to hundreds of kilobytes or several megabytes of repetitive source text, headers, declarations, and manifests.

Three technical constraints govern any compression choice in `playtest-assets`:

1. **Pure safe Rust across all five targets.** The workspace enforces `unsafe_code = "forbid"` and cross-compiles static musl Linux (`x86_64`, `aarch64`), macOS (`x86_64`, `aarch64`), and Windows (`x86_64-pc-windows-msvc`) binaries without C toolchains or FFI linkers.
2. **License and MSRV compliance.** Every crate must satisfy `deny.toml` (`MIT` / `BSD-3-Clause` / `Apache-2.0` allowlist) and `playtest-assets`'s `rust-version = "1.85"` floor.
3. **Break-even binary footprint and cross-platform determinism.** Linking a heavy compressor into the shipped CLI wastes binary size, while `brotli-decompressor` carries a ~150–200 KB fixed footprint (safe decoder plus the ~120 KB RFC 7932 static dictionary). Below ~200 KB of raw SDK sources (as in M1 `v0.1.0`), uncompressed `include_dir` produces a smaller binary; once full polyglot SDKs land in M5, solid cross-file Brotli compression achieves 4×–7× reduction on source code and pays back the decoder overhead many times over.

## Decision

1. **Adopt solid `.tar.br` compression in `playtest-assets` at Milestone M5 (`task_067`).**
   - `crates/playtest-assets/build.rs` packs the normalized published SDK files in deterministic lexicographic order into one POSIX `tar` stream and compresses that stream with `brotli` (`9.x`, quality `11`, `lgwin = 22`, feature `portable-float` enabled so cost-model math is target-independent and byte-identical across host OSes) into `$OUT_DIR/embedded_sdks.tar.br`, embedded via `include_bytes!`.
   - Solid stream compression across all SDK files is chosen over per-file `.br` blobs so Brotli's sliding window and RFC 7932 static dictionary share repeated tokens across small files and across languages.
2. **Split build-time encoder from runtime decoder.**
   - `brotli` (`features = ["portable-float"]`) is declared only in `[build-dependencies]` of `playtest-assets` and is never linked into `playtest-cli`.
   - `brotli-decompressor` (`6.x`, pure safe Rust, `BSD-3-Clause OR MIT`, MSRV `1.59`) is declared in `[dependencies]` of `playtest-assets` when `task_067` activates `.tar.br` storage.
3. **Preserve the zero-copy `'static` public API via `OnceLock`.**
   - `playtest-assets` lazily decompresses and indexes `$OUT_DIR/embedded_sdks.tar.br` once on first access behind `std::sync::OnceLock`, keeping `EmbeddedFile { path: &'static str, contents: &'static [u8] }` unchanged for `playtest-cli`.
4. **Keep M1 (`v0.1.0`) uncompressed while pre-staging the crates in `Cargo.lock` and the offline sandbox.**
   - Because M1's scaffolded SDKs total ~25 KB raw (below `brotli-decompressor`'s ~150–200 KB fixed cost), `v0.1.0` ships with uncompressed `include_dir` while declaring `brotli` and `brotli-decompressor` in workspace/dev dependencies now so `relock.yml` and `publish-sandbox.yml` vendor `brotli`, `brotli-decompressor`, `alloc-no-stdlib`, and `alloc-stdlib` into `Cargo.lock` and `sandbox/developer-linux-64`.
5. **Keep consumer mirror formats unchanged.**
   - `playtest sync` continues rendering standard PEP 427 `.whl` (Stored ZIP) and npm `.tgz` (Gzip tar) artifacts from the decompressed in-memory `EmbeddedFile` slices ([ADR 0017](./0017-committed-sdk-mirrors-and-explicit-wiring.md)).

## Alternatives Rejected

- **Per-file Brotli blobs inside `include_dir`.** Resets the sliding window on every 200 B–2 KB file, losing cross-file dictionary sharing across manifests, license headers, and shared fixture/BDD terminology.
- **Linking the full `brotli` crate into `[dependencies]`.** Pulls the LZ77 backward-reference encoder into the shipped `playtest` CLI binary even though runtime only ever decompresses.
- **C-backed compression (`zstd-sys`, `brotli-sys`, `liblzma`).** Violates the pure-Rust `unsafe_code = "forbid"` policy and complicates static musl and cross-OS builds.
- **Switching to `.tar.br` in `v0.1.0` before M2–M5 SDK sources exist.** Adds ~150–200 KB of decoder + static dictionary to save ~18 KB on version-only stubs, temporarily increasing binary size instead of shrinking it.

## Consequences

### Positive
- Scales cleanly as all eight embedded SDKs grow across M2–M5 without bloating the single `playtest` executable.
- `portable-float` keeps compile-time `.tar.br` output deterministic across Linux, macOS, and Windows builders.
- Pre-locking and vendoring `brotli` + `brotli-decompressor` now makes `task_067` executable in airgapped developer sandboxes without network access.

### Negative
- First call to `embedded_files` / `get_embedded_file` in a `playtest sync` or `playtest version` invocation pays a sub-millisecond one-time in-memory decompression cost and retains the decompressed SDK bytes for the process lifetime (negligible for short-lived CLI commands).
