# Specification 0013: Workspace Configuration, SDK Mirrors, and Wiring

## Overview

Specifies the files the CLI owns in a consumer repository, the commands that create and refresh them, and how a language project is connected to its SDK, as decided in [ADR 0017](../adr/0017-committed-sdk-mirrors-and-explicit-wiring.md). Target milestone: M1 (`v0.1.0`) for the commands and for the Python and TypeScript mirrors; the other targets follow in M5 (`v0.5.0`).

**Implementation status:** `playtest.toml`, `playtest version`, `playtest init`, and the §6 version pin check are implemented (`task_052`). `playtest sync` and the SDK mirrors follow in `task_053` and `task_064`.

Terms:

- **Workspace**: a directory tree with a `playtest.toml` at its root, normally a repository.
- **Mirror**: a copy of one language's SDK, written by the CLI from its embedded files ([Spec 0012 §2](./0012-workspace-crates-and-cli-distribution.md)) into `.playtest/<lang>/`, in the form that language's installer accepts.
- **Wiring**: the one-time change to a language project's manifest that makes it install the SDK from the mirror.

## 1. `playtest.toml`

- The file sits at the workspace root. A command locates the workspace by the nearest `playtest.toml` at or above the current directory.
- Settled `schema = 1` keys (`playtest_core::WorkspaceConfig`):

  | Key | Type | Required | Meaning |
  | :--- | :--- | :--- | :--- |
  | `schema` | integer | Yes | Manifest schema version; must equal `1`. Unsupported schema numbers fail with exit status `3`. |
  | `version` | non-empty string | Yes | Exact Playtest SemVer release pinned by the workspace (for example `"0.1.0"`). |
  | `mirrors` | table | No | Mirror configuration table; defaults to empty when omitted. |
  | `mirrors.languages` | array of strings | No | Canonical SDK target identifiers mirrored into `.playtest/<lang>/` by `playtest sync` (`["python", "typescript"]` by default when created by `playtest init`, overridable via `playtest init --lang <target>`). |

  Unknown keys in `schema = 1` are rejected as invalid workspace configuration (`exit status 3`) so typos cannot go unnoticed.

- Canonical layout written by `playtest init`:

  ```toml
  schema = 1
  version = "0.1.0"

  [mirrors]
  languages = ["python", "typescript"]
  ```

- `playtest.toml` is the intended home of later workspace settings, such as the doctor probes that [Spec 0008 §8](./0008-cross-language-cli-and-toolchain-detection.md) registers through "workspace configuration". `task_037` confirms or changes this.

## 2. `playtest init`

1. Creates `playtest.toml`, pinned to the version of the running binary.
2. Creates `.playtest/` with a `.gitignore` that re-includes mirror artifacts (`!*.whl`, `!*.tgz`), so that repository-wide ignore rules for those extensions do not hide them. This file cannot undo a rule that ignores the `.playtest/` directory itself. Such a repository commits no mirror, and `playtest sync --check` fails on a fresh clone.
3. Writes nothing else. It never edits `pyproject.toml`, `package.json`, `go.mod`, `pixi.toml`, a lockfile, or any other file it did not create.
4. Is idempotent. A second run changes nothing and does not overwrite an edited `playtest.toml`.
5. Prints the next steps: `playtest sync` and the wiring commands of §5.

## 3. `playtest sync`

1. Writes the mirror of each configured language from the files embedded in the running binary.
2. Is deterministic: the same CLI version writes byte-identical files on every supported platform. Archive timestamps, entry order, and compression settings are fixed for this purpose.
3. Replaces the mirror of the previous version and leaves no other file in `.playtest/<lang>/`.
4. Prints the wiring command for a language whenever the path of its mirror changes. It never runs that command.
5. `playtest sync --check` writes nothing. It exits with status 0 when `.playtest/` equals what `sync` would write. Otherwise it exits with status 3, lists every missing, stale, or modified file, and names the fix (`playtest sync`).
6. Mirrors are committed to Git, and CI runs `playtest sync --check`.

## 4. Mirror Forms

| Language | Mirror in `.playtest/<lang>/` | Package name | State on 2026-11-11 |
| :--- | :--- | :--- | :--- |
| Python | `py3-none-any` wheel | `playtest-py`, imported as `playtest` | Prototyped with uv |
| TypeScript | npm tarball (`.tgz`) | `@archont561/playtest` | Prototyped with npm, pnpm, Bun, and Yarn |
| Go | module directory | `github.com/Archont561/playtest/go` | Prototyped with Go 1.27 |
| Rust | none; crates.io | `playtest` | Not applicable |
| C# | to be designed; a project directory added with `dotnet add reference` is the candidate | `Playtest` | Not tested |
| Java, R, C, C++ | to be designed | — | Not designed |

"Prototyped" means tried in scratch projects outside this repository. No repository test covers a mirror yet.

Rules:

1. A mirror has the same package name and import path as the registry package. Test code does not change with the source of the SDK.
2. File names follow the ecosystem's convention and carry the version: `playtest_py-<version>-py3-none-any.whl` and `archont561-playtest-<version>.tgz`. A new version is therefore a new path, and the Python and TypeScript wiring is repeated on every upgrade (§5).
3. A mirror holds the SDK only. It does not hold the binary.
4. A target appears as supported only after its mirror form and wiring are recorded here and covered by an offline install test as defined in §9 (`task_053` for Python and TypeScript, `task_064` for the others).

## 5. Wiring

The user runs the wiring command once per project, and again when the path of the mirror changes.

| Language | Command |
| :--- | :--- |
| Python | `uv add --dev <path to wheel>` |
| TypeScript | `npm add --save-dev <path to tarball>`, `pnpm add --save-dev <path to tarball>`, or `bun add --dev <path to tarball>` |
| Go | `go mod edit -replace github.com/Archont561/playtest/go=./.playtest/go`, or a `use` line in `go.work` |
| Rust | `cargo add playtest --dev` |

A project may depend on the registry package at exactly the pinned version instead of the mirror. The handshake of [Spec 0014 §7](./0014-report-protocol-and-runner-orchestration.md) detects a different version at run time.

**Upgrade.** The steps are: change the pin, run `playtest sync`, repeat the wiring command in each Python and TypeScript project with the new path, and commit the mirrors, manifests, and lockfiles together.

`playtest sync --check` compares `.playtest/` with the running binary. It does not look at what a project has installed. The table shows what an install did on 2026-11-11 when the wiring command had been forgotten: `sync` had replaced the `0.1.0` file with the `0.2.0` file, and the manifest and lockfile still named the `0.1.0` file. The package was a stand-in with the SDK's name, in scratch projects outside this repository.

| Package manager | Command | Package cache holds the old file | Package cache is empty |
| :--- | :--- | :--- | :--- |
| uv 0.13.7 | `uv sync`, `uv sync --locked` | fails | fails |
| pnpm 12.18.0 | `pnpm install --frozen-lockfile` | fails | fails |
| pnpm 12.18.0 | `pnpm install` | exits 0, installs the old SDK | fails |
| npm 10.9.8 | `npm ci`, `npm install` | exits 0, installs the old SDK | fails |
| Bun 1.4.4 | `bun install`, `bun install --frozen-lockfile` | exits 0, installs the old SDK | fails |

A developer machine is in the first column, and so is a CI job that restores the package cache. There, npm and Bun do not report the forgotten step. From M5, the handshake of Spec 0014 §7 reports the old SDK when `playtest test` runs. Before M5 nothing in Playtest reports it (§8, question 3).

## 6. Version Pin Check

1. Every command run inside a workspace, except `playtest version`, first compares the version of the running binary with the pin in `playtest.toml`.
2. On a mismatch the command exits with status 3. The message names both versions, the path of the running binary, and the two fixes: install the pinned version, or change the pin and run `playtest sync`.
3. `playtest version` is exempt so that a mismatch can be diagnosed. It always prints the version and path of the running binary, and it reports a mismatch with the pin as a warning.
4. The CLI does not search for another binary. When several are installed, for example one from npm and one from a pip wheel, the one found first on `PATH` must match the pin.

## 7. Reference Scenarios

| Repository | Binary arrives through | SDKs arrive through |
| :--- | :--- | :--- |
| Pixi or conda monorepo with several languages | conda package in the Pixi environment | Mirrors, wired per project; Rust from crates.io |
| npm monorepo that also holds Python packages | `@archont561/playtest` | TypeScript SDK inside the same npm package; Python mirror wired with `uv add` |
| Go-only repository | `go get -tool …/go/cmd/playtest@vX.Y.Z` | The same Go module at the same version; no mirror |
| Rust-only repository | `cargo install playtest-cli`, if the CLI is wanted at all | `playtest` from crates.io; no mirror |
| One machine with a binary from npm and one from a pip wheel | Whichever is first on `PATH` | The pin check of §6 rejects the wrong one |

## 8. Open Questions

1. Whether `sync` should also offer a flat package index for Python, so that the wiring survives upgrades.
2. The mirror forms and wiring commands for C#, Java, R, C, and C++.
3. How a forgotten wiring command is detected before M5 (§5). One candidate: `playtest sync --check` also reads the project manifests and fails when one names a file in `.playtest/` that no longer exists. Reading a manifest does not conflict with §2, which forbids editing one. `task_053` decides.

## 9. Verification

The implementing tasks provide at least:

- an offline install test for each package manager named in §5: from a fresh clone, without the CLI and without network access, the package manager installs the SDK from its mirror. Third-party dependencies of the SDK, such as pytest, may come from a package cache that was filled beforehand;
- `sync --check` tests for a missing, a stale, and a modified mirror;
- an upgrade test for each package manager named in §5: after `sync` replaces a mirror and the wiring command is repeated, the new SDK is installed;
- a test that two runs of `sync` on different platforms write identical bytes;
- tests for a first and a repeated `init`, a matching pin, a mismatched pin, and a missing `playtest.toml`.
