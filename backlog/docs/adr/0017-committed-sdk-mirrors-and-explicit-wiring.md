# ADR 0017: Committed SDK Mirrors and Explicit Wiring

## Status
Accepted (2026-11-11).

## Context

A repository installs the CLI through one channel ([ADR 0016](./0016-cli-binary-distribution-channels.md)) but may test in several languages. An npm monorepo that also holds Python packages gets the binary and the TypeScript SDK from npm. Its Python tests still need the Python SDK, at the same version. Installing that SDK from PyPI would bring back a second, independently chosen version, which [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md) set out to remove.

The CLI embeds every SDK, so it can write them out. The open points were where the copies live, in what form, and who edits the consumer's manifests.

Two terms are used below and in [Spec 0013](../specs/0013-workspace-configuration-and-sdk-mirrors.md):

- A **mirror** is a copy of one language's SDK that the CLI writes into the repository, in the form that language's installer accepts.
- **Wiring** is the one-time change to a project's manifest that makes it install the SDK from the mirror.

## Decision

1. **`playtest sync` writes mirrors into `<repo>/.playtest/<lang>/`**, and the mirrors are committed to Git.
2. **A mirror is an installer-native artifact**: a `py3-none-any` wheel for Python, an npm tarball for TypeScript, a module directory for Go. It has the same package name and import path as the registry package, so test code is the same whichever source it was installed from. The file name of a wheel or a tarball carries the version, so a new version is a new path.
3. **`playtest sync --check` is the drift gate.** It writes nothing and fails when `.playtest/` differs from what the running CLI would write. CI runs it.
4. **`playtest init` writes only files that Playtest owns**: `playtest.toml` and `.playtest/`. It never edits `pyproject.toml`, `package.json`, `go.mod`, `pixi.toml`, or a lockfile.
5. **Wiring is an explicit step with the ecosystem's own command**, for example `uv add`, `npm add`, or `go mod edit -replace`. The CLI prints the command and does not run it. The step is repeated in each project when the path of its mirror changes, which for Python and TypeScript is every upgrade.
6. **`playtest.toml` pins the Playtest version.** Whichever `playtest` binary is found first on `PATH` compares its own version with the pin and exits with status 3 on a mismatch. This settles the case of several installed binaries, such as one from npm and one from a pip wheel.
7. **Rust needs no mirror.** Rust projects take `playtest` from crates.io.
8. **Mirrors never live outside the repository.** The CLI keeps no machine-wide configuration and no state in `/tmp`.

## Alternatives Rejected

Each was tried in scratch projects on 2026-11-11.

- **Mirrors under `/tmp/.playtest/`.** The path does not exist on another machine, in CI, or after a reboot, and manifests and lockfiles would record it.
- **Mirrors that are generated but not committed.** `npm install`, `pnpm install`, `bun install`, `yarn install`, and `uv sync` all fail when a path dependency is missing, so a fresh clone cannot install until the CLI has run. In an npm-only repository the CLI itself arrives through that install.
- **A tarball whose path stays the same across versions.** After its content was replaced, npm 10.9.8 and Bun 1.4.4 kept installing the old SDK from their caches and reported success, and nothing in the manifest showed that the project had been upgraded.
- **A Python source tree as the mirror.** Installing it needs a build backend, which the installer downloads. That fails offline. A built wheel needs no backend.
- **`NODE_PATH`, or a Node `imports` entry that points into `.playtest/`.** Node ignores `NODE_PATH` for ES modules, and `imports` targets must stay inside the package.
- **Importing the mirror under an alias.** Unnecessary once the mirror keeps the registry package's name, and it would make test code depend on how the SDK was installed.
- **Letting `init` edit consumer manifests.** Every package manager has its own manifest rules and lockfile, and a tool that rewrites them owns every resulting conflict.

## Consequences

### Positive
- A fresh clone installs the SDK with nothing but the package manager: no Playtest binary, no build step, and no network access for the SDK itself. Third-party dependencies of an SDK, such as pytest, are still resolved by the package manager.
- The version of a mirror equals the version of the CLI that wrote it, and an upgrade shows up as a reviewable diff.
- The consumer's manifests change only by the consumer's own commands.

### Negative
- Wheels and tarballs are committed to consumer repositories. Repositories that ignore `*.whl` or `*.tgz` need the `.playtest/.gitignore` that `init` writes to re-include them.
- An upgrade has four steps: change the pin, run `playtest sync`, repeat the wiring command in every Python and TypeScript project, and commit. `playtest sync --check` catches a forgotten `sync`. It does not catch a forgotten wiring command.
- A forgotten wiring command can pass unnoticed. In tests on 2026-11-11 the old tarball was replaced by the next version while the manifest and lockfile still named the old file. npm 10.9.8 and Bun 1.4.4 then installed the old SDK from their caches and exited with status 0. They failed only when the cache was empty. pnpm 12.18.0 failed with `--frozen-lockfile` and behaved like npm without it. uv 0.13.7 failed in every case ([Spec 0013 §5](../specs/0013-workspace-configuration-and-sdk-mirrors.md)).
- The mirror is not the installed SDK. Until the run-time check of [ADR 0018](./0018-sdk-reporters-and-report-protocol.md) exists (M5), no Playtest command detects an installed SDK that is older than its mirror (`task_053`).
- Only the Python, TypeScript, and Go mirror forms were prototyped. The forms for C#, Java, R, C, and C++ are not designed yet (`task_064`).
