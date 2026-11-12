# Specification 0012: Workspace Crates, Embedded Assets, and CLI Distribution

## Overview

Specifies the Rust workspace layout, the embedding of SDK files, and the channels through which the `playtest` binary reaches users, as decided in [ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md) and [ADR 0016](../adr/0016-cli-binary-distribution-channels.md). Target milestone: M1 (`v0.1.0`).

**This is a target contract. None of it is implemented or published yet.** Delivery is tracked by `task_050` to `task_059`. Statements about third-party tools reflect documentation and experiments from 2026-11-11 and must be confirmed by the implementing task.

Terms:

- **Binary**: the `playtest` executable built from the `playtest-cli` crate.
- **SDK**: the library that test code imports in one language, kept in that language's directory (`python/`, `typescript/`, `go/`, …).
- **Wrapper**: a registry package that locates or downloads the binary. It contains no CLI logic.

## 1. Workspace Crates

| Crate | Kind | Workspace dependencies | Purpose | crates.io name |
| :--- | :--- | :--- | :--- | :--- |
| `playtest` | library | `playtest-macros`, `playtest-core` | Rust SDK; the only crate test code declares | reserved |
| `playtest-macros` | proc-macro | — | Macros re-exported by `playtest` | reserved |
| `playtest-core` | library | — | Types shared by the Rust SDK and the CLI: report-protocol records ([Spec 0014](./0014-report-protocol-and-runner-orchestration.md)) and version constants | to reserve |
| `playtest-assets` | library | — | Embedded SDK files (§2) | to reserve |
| `playtest-cli` | binary `playtest` | `playtest-core`, `playtest-assets` | The CLI, built on `clifacet` | to reserve |

Rules:

1. `playtest-cli` is the only crate that depends on `playtest-assets` or on `clifacet`.
2. `playtest` must not depend on `playtest-assets` or `playtest-cli`, directly or transitively. A test or lint enforces this.
3. Every crate exports `VERSION`, and all five carry the workspace version.
4. Crates are published in dependency order: `playtest-macros`, `playtest-core`, `playtest-assets`, `playtest`, `playtest-cli`.
5. Building `playtest-cli` needs Rust `1.88` or newer, the floor of `clifacet` `0.5.0`. Each library crate declares its minimum Rust version separately in its own `Cargo.toml`: `playtest` declares `1.88` (the floor of `proptest` `1.12`), while `playtest-macros`, `playtest-core`, and `playtest-assets` declare `1.85` (the Rust 2024 edition floor).
6. `clifacet` types appear neither in `playtest-core` nor in documented CLI output. The output and exit-status contract is [Spec 0008 §7](./0008-cross-language-cli-and-toolchain-detection.md).

## 2. Embedded SDK Assets (`playtest-assets`)

1. The language directories are the only source of SDK files. `playtest-assets` holds no second copy in Git.
2. An explicit file list per language decides what is embedded. The list names the files that the SDK's own package publishes. Tests, lockfiles, caches, and editor files are never listed.
3. Where a package publishes build output, that output is built before `playtest-assets`. The TypeScript package publishes `dist/`, so the TypeScript build precedes the Rust build in a release.
4. `build.rs` copies the listed files into `OUT_DIR`, and the crate embeds that directory with `include_dir`. The crate creates no symbolic link and follows none.
5. `cargo package` includes only files inside the crate directory. Before packaging, a Python module under `automation/` (`automation/stage_assets.py`; [ADR 0009](../adr/0009-python-only-repo-automation.md)) copies the listed files into the crate directory. The packaged crate must build without the rest of the repository.
6. The crate exposes, per language, the embedded files with their relative paths, and the SDK version. That version equals the workspace version.
7. A test fails when an embedded file differs from its source or when a listed file is missing.
8. The crate embeds files, not installable artifacts. The CLI renders wheels, tarballs, and module directories from them when `playtest sync` runs ([Spec 0013 §3](./0013-workspace-configuration-and-sdk-mirrors.md)).

## 3. GitHub Release Artifacts

Every other channel and both download wrappers depend on the GitHub Release.

1. cargo-dist builds the binary for each platform of the supported matrix. The matrix is declared here by `task_054`; it is not fixed yet.
2. Archives are `.tar.gz` on Unix and `.zip` on Windows. cargo-dist's Unix default is `.tar.xz` and is overridden.
3. Each archive has a `.sha256` file, and the release has one combined `sha256.sum`.
4. The release tag is `vX.Y.Z`. Every download URL can be derived from the version and the platform alone.
5. The release is published with all assets attached. Assets are never added or replaced afterwards.
6. GitHub immutable releases are enabled for the repository. This is a repository setting made by an admin, not part of a workflow file. The setting could not be read from the agent sandbox on 2026-11-11 (the API returned 403), so its current state is unknown.

## 4. Registry Channels

### 4.1 crates.io

- `cargo install playtest-cli` builds from source and installs a binary named `playtest`. It needs Rust 1.88 or newer.
- All five crates are published through Trusted Publishing ([ADR 0006](../adr/0006-crates-io-trusted-publishing.md)). `playtest-core`, `playtest-assets`, and `playtest-cli` are first reserved at `0.0.0`, as `playtest` and `playtest-macros` were.

### 4.2 PyPI

- Package `playtest-py`, built by maturin with `bin` bindings. Each wheel holds the binary and the importable Python package `playtest`.
- One wheel per platform of the matrix. The source distribution builds only with Rust 1.88 or newer and says so when the build fails.
- `pip install playtest-py` puts `playtest` on `PATH` and makes `import playtest` work, both at the release version.
- The package depends on no CLI framework. `typer` is removed.
- The wheel declares the pytest plugin entry point for the reporter of [Spec 0014 §4](./0014-report-protocol-and-runner-orchestration.md) once that reporter exists.

### 4.3 npm

- Platform packages `@archont561/playtest-<platform>-<arch>` each hold one binary and declare `os` and `cpu`. cargo-npm generates them.
- The wrapper package `@archont561/playtest` lists the platform packages as `optionalDependencies` and also carries the TypeScript SDK and the loader.
- No package declares a lifecycle script.
- Linux binaries for npm are static musl builds, so there is one Linux package per architecture and no libc selection.
- Configuration sketch for `crates/playtest-cli/Cargo.toml`. `task_056` settles the values and adds the `targets` list from the platform matrix:

  ```toml
  [package.metadata.npm]
  name = "@archont561/playtest"
  prefix = "@archont561/playtest-"
  bins = ["playtest"]
  mode = "merge"
  custom = { engines = { node = ">=18" } }
  ```

- Generation steps, run by a Python script: stage the built TypeScript SDK into `npm/@archont561/playtest/`; run `cargo npm generate`; replace the generated `bin/playtest.js` with the loader; add `bin/package.json` containing `{"type":"commonjs"}`; list `bin` in `files`.
- Publication: `cargo npm publish -- --provenance --access public`. Platform packages are published before the wrapper.
- Loader contract (`bin/playtest.js`):
  1. Resolve `@archont561/playtest-<platform>-<arch>` for the host.
  2. If the package is absent or its binary is not executable, exit with status 3 and a message that names the package and the fix. Print no stack trace.
  3. On Unix, replace the loader's process with the binary where the runtime offers `process.execve`.
  4. Otherwise start the binary with inherited standard streams, forward signals to it, and exit with its status. When a signal ends the binary, end the loader the same way instead of exiting with status 1.
  5. Work whether or not the wrapper package declares `"type": "module"`.
- Known npm behaviour: npm 10.9.8 wrote a lockfile that listed one of six platform packages when the lockfile was regenerated over an existing `node_modules`. Consumer documentation tells users to regenerate lockfiles from a clean tree.

### 4.4 conda

- A recipe built with `pixi-build-rust` produces the package `playtest`, which puts the binary on `PATH`.
- The receiving channel is chosen by `task_057`. The root `pixi.toml` already lists the channel `https://prefix.dev/archont561/archont561`.

## 5. Download Wrappers (Go and R)

### 5.1 Common Contract

1. A wrapper published at version X.Y.Z downloads only the X.Y.Z archive for the host operating system and architecture, from the GitHub Release `vX.Y.Z`.
2. It downloads the checksum file of that release and verifies the archive's SHA-256 before unpacking.
3. It keeps the binary in a per-user cache, keyed by version and platform, and reuses it. A cached binary was verified when it was downloaded and is not hashed again on later runs. The wrapper writes nothing into the repository or a machine-wide configuration.
4. The first use needs network access. Later runs do not.
5. A missing release, an unsupported platform, a checksum mismatch, and a network failure each produce an error that names the URL and the fix. The Go wrapper exits with status 3; the R function raises an R error. A wrapper never falls back to another version.
6. The checksum file comes from the same release as the archive, so the check protects against corruption and partial downloads. Protection against a replaced release rests on immutable releases (§3).

### 5.2 Go

- The command lives at `go/cmd/playtest` in the existing module `github.com/Archont561/playtest/go`.
- Install with `go install github.com/Archont561/playtest/go/cmd/playtest@vX.Y.Z`, or record it as a module tool with `go get -tool github.com/Archont561/playtest/go/cmd/playtest@vX.Y.Z`. `go get -tool` needs Go 1.24 or newer; `go/go.mod` declares `go 1.22` today.
- On Unix the wrapper replaces its own process with the binary. On Windows it starts the binary as a child process and passes arguments, standard streams, and the exit status through. Only the Unix path was prototyped.
- The module uses the standard library only. `cobra` and its indirect requirements are removed from `go/go.mod`.
- The module version comes from the tag `go/vX.Y.Z` ([Spec 0006](./0006-two-phase-release-and-trusted-publishing.md)).

### 5.3 R

- `playtest::install_cli()` downloads the binary when the user calls it. Nothing is downloaded when the package is installed or loaded, as CRAN policy requires.
- The binary is stored under `tools::R_user_dir()`.
- `tools::sha256sum()` exists only from R 4.5. The package documents how it verifies the checksum on older R, or raises its minimum R version.
- `playtest-r` is not a valid R package name. The package is named `playtest`.

## 6. Deferred Channels

Conan, Maven Central, and NuGet get no binary package in `v0.1.0`. Java, C#, C, and C++ repositories install the binary through GitHub Releases, conda, or another channel of §4. What is known:

- **Conan**: a package created with `export-pkg` and consumed through `tool_requires` worked in a prototype.
- **Maven Central**: per-platform executables can be published as classified artifacts, as `com.google.protobuf:protoc` does. Not tested.
- **NuGet**: .NET 10 supports tool packages per runtime identifier. Not tested.

## 7. Package Names

Checked on 2026-11-11 against each registry's public index or API. "Owned by this project" means the registry lists the maintainer's account as owner, author, or maintainer of a `0.0.0` reservation.

| Registry | Name | State |
| :--- | :--- | :--- |
| crates.io | `playtest`, `playtest-macros` | Owned by this project |
| crates.io | `playtest-core`, `playtest-assets`, `playtest-cli` | Free; to reserve |
| PyPI | `playtest-py` | Owned by this project |
| PyPI | `playtest` | Taken by an unrelated project |
| npm | `@archont561/playtest` | Owned by this project; platform packages use the same scope |
| npm | `playtest-ts` | Owned by this project; not used by this design |
| npm | `playtest` | Taken by an unrelated project |
| NuGet | `Playtest` | Owned by this project |
| Maven Central | `io.github.archont561:playtest` | Owned by this project (`0.0.0` published) |
| CRAN | `playtest` | Free; not submitted |
| Go | `github.com/Archont561/playtest/go` | Module path; no reservation needed |
| conda-forge | `playtest` | No feedstock found; the receiving channel is not chosen (§4.4) |
| ConanCenter, vcpkg | `playtest-cpp`, `playtest-c` | No recipe found; this project publishes to a configured Conan remote |

## 8. Release Pipeline Requirements

1. **One version per release.** A check fails the release when any package manifest, `VERSION` constant, or embedded SDK version differs from the release version.
2. **Build in Phase 1.** Every artifact is built in Phase 1 from the release tag and stored as a workflow artifact. Publishing jobs upload and never compile ([ADR 0008](../adr/0008-two-phase-artifact-release-pipeline.md)). Which channels repackage one compiled binary and which compile the tag themselves is settled by `task_055` to `task_057`.
3. **Order.** npm platform packages are published before the wrapper. The GitHub Release is published with its assets before the tag `go/vX.Y.Z` is pushed and before an R package version is submitted.
4. **No wrapper without a binary.** No Go or R version is published without a binary release of the same version. A Go-only release tag has no such release today; `task_059` resolves this.
5. **Automation rules.** Helpers are Python scripts under `scripts/` ([ADR 0009](../adr/0009-python-only-repo-automation.md)), and commands run through Pixi ([ADR 0005](../adr/0005-hermetic-pixi-and-nextest-toolchain.md)).
6. **Platforms.** The Pixi workspace declares only `linux-64` today. The release build covers every platform of the matrix in §3.

## 9. Verification

The implementing tasks provide at least:

- a test of the dependency rules in §1;
- a build of the packaged `playtest-assets` crate outside the repository, and a comparison of embedded files with their sources;
- loader tests for a missing platform package, a non-executable binary, a wrapper with `"type": "module"`, termination by signal, and exit-status passthrough;
- wrapper tests for a missing release, a checksum mismatch, a network failure, and an unsupported platform;
- for each channel, an install followed by `playtest version` that prints the release version.
