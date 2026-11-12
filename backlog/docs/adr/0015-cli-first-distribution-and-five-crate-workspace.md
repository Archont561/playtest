# ADR 0015: CLI-First Distribution with Embedded SDKs and a Five-Crate Workspace

## Status
Accepted (2026-11-11). Supersedes [ADR 0001](./0001-two-crate-monorepo-architecture.md). Amends [ADR 0006](./0006-crates-io-trusted-publishing.md), [ADR 0007](./0007-polyglot-monorepo-architecture.md), and [ADR 0011](./0011-cross-language-cli-and-compatibility-architecture.md). Puts the scoped release tags of [ADR 0010](./0010-milestone-driven-autorelease-lifecycle.md) under review (open question 1).

## Context

[ADR 0007](./0007-polyglot-monorepo-architecture.md) gave each of the nine language targets its own package, manifest, and registry release. [ADR 0011](./0011-cross-language-cli-and-compatibility-architecture.md) planned one shared CLI for Milestone M5. Together they leave ten separately versioned things: nine SDKs and the CLI. Three problems follow.

1. **Version skew.** A repository that tests in several languages needs the same Playtest version in each of them. The old model cannot detect a Python SDK at one version beside a TypeScript SDK or a CLI at another.
2. **Uneven registries.** Publishing to Conan, NuGet, and Maven Central runs only when credentials are configured, and CRAN submission is manual. A consumer cannot count on every SDK being on its registry on release day.
3. **CLI drift.** The Python, TypeScript, and Go packages declare the CLI frameworks `typer`, `commander`, and `cobra`, which no code uses, although [Spec 0005 §4](../specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md) already requires one shared tool.

`v0.1.0` has not been tagged or published, so the model can still change without breaking a user.

## Decision

1. **One binary is the version authority.** The Rust CLI `playtest` is released at one version and embeds the source of every language SDK at that version. A release is one version across the CLI and all SDKs.
2. **The workspace has five crates** ([Spec 0012 §1](../specs/0012-workspace-crates-and-cli-distribution.md)):

   | Crate | Kind | Role |
   | :--- | :--- | :--- |
   | `playtest` | library | The crate that test code imports; re-exports the macros |
   | `playtest-macros` | proc-macro | Procedural macros, kept separate as ADR 0001 required |
   | `playtest-core` | library | Types shared by the Rust SDK and the CLI, such as the report-protocol records ([ADR 0018](./0018-sdk-reporters-and-report-protocol.md)) |
   | `playtest-assets` | library | The SDK files of every language, embedded with `include_dir` |
   | `playtest-cli` | binary `playtest` | The CLI |

3. **Dependency rule.** `playtest-cli` is the only crate that depends on `playtest-assets` and on `clifacet`. `playtest` depends on neither, so a test build never compiles the CLI or the embedded SDKs. Test code still declares only `playtest`.
4. **The CLI is built with [`clifacet`](https://github.com/Archont561/clifacet)**, the maintainer's CLI library over `clap`. `clifacet` writes no output itself, so the output and exit-status contract of [Spec 0008 §7](../specs/0008-cross-language-cli-and-toolchain-detection.md) is implemented in `playtest-cli`.
5. **Language directories stay the single source of each SDK.** `python/`, `typescript/`, `go/`, and the other directories of ADR 0007 keep their native manifests. They gain a second role: `playtest-assets` embeds them.
6. **Language packages carry no CLI of their own.** The only per-language CLI code is a wrapper that locates or downloads the one binary ([ADR 0016](./0016-cli-binary-distribution-channels.md)).
7. **Version skew is an error.** A workspace pins its Playtest version in `playtest.toml` ([ADR 0017](./0017-committed-sdk-mirrors-and-explicit-wiring.md)), and each SDK states its version when a run starts ([ADR 0018](./0018-sdk-reporters-and-report-protocol.md)). A mismatch ends the command with exit status 3.
8. **The CLI ships from `v0.1.0`.** Milestone M1 delivers the binary with `version`, `init`, and `sync`. Milestone M5 adds `list`, `doctor`, `test`, and `mutation`.

## Alternatives Rejected

- **Nine independent package versions plus a compatibility matrix.** Skew stays possible, and every pair of versions needs a documented answer.
- **A CLI in each language package.** Nine implementations would have to behave alike. This is the direction the unused `typer`, `commander`, and `cobra` dependencies pointed in.
- **Assets and CLI inside the `playtest` crate.** Every test build would compile the SDK files of nine languages and the CLI's dependencies.

## Consequences

### Positive
- There is one version to reason about, and a mismatch is reported instead of going unnoticed.
- One install gives a repository the CLI and every SDK at the same version.
- The proc-macro boundary of ADR 0001 is kept: users still declare only `playtest`.

### Negative
- A change to any SDK needs a new CLI release, so all artifacts move together.
- The binary grows with every embedded SDK.
- Building the CLI from source needs Rust 1.88 or newer, the floor of `clifacet` 0.5.0. The root `pixi.toml` allows `rust >=1.82.0`. Its lock file resolves Rust 1.99.0 today, so the hermetic build works, but the bound understates the floor.
- `clifacet` 0.5.0 describes itself as experimental. Its version must be pinned, and its types must stay out of documented output.
- Five crates are published in dependency order, and three new crate names must be reserved.

## Open Questions

1. **Scoped release tags.** [Spec 0007](../specs/0007-autorelease-change-detection-and-milestone-dispatch.md) cuts `<scope>-vX.Y.Z` and `go/vX.Y.Z` tags that release one language at its own version. That conflicts with decision 1. `task_059` decides whether scoped tags are retired or kept for a narrower purpose.
2. **Versions after `v1.0.0`.** [ADR 0014](./0014-v1-stability-boundary-and-fixture-extension-model.md) makes core package releases fixes-only after 1.0. With one version per release, a CLI feature release also raises every SDK's version number. Whether "fixes-only" then describes SDK content rather than SDK version numbers is undecided.
3. **Minimum Rust version of the library crates.** `playtest`, `playtest-macros`, and `playtest-core` do not depend on `clifacet` and can declare a lower floor than the CLI. The value is set by `task_050`.
