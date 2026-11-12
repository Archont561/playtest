<!--
  Generated file - do not edit README.md by hand.

  Source template : automation/readme/README.ejs
  Generator       : readme-md-generator (pinned in package.json)
  Regenerate with : pixi run doit readme        (python -m automation.readme)
  Verify in CI    : pixi run doit readme_check  (python -m automation.readme --check)
-->

<h1 align="center">Playtest</h1>

<pre style="font-family: monospace">
██████╗ ██╗      █████╗ ██╗   ██╗████████╗███████╗███████╗████████╗
██╔══██╗██║     ██╔══██╗╚██╗ ██╔╝╚══██╔══╝██╔════╝██╔════╝╚══██╔══╝
██████╔╝██║     ███████║ ╚████╔╝    ██║   █████╗  ███████╗   ██║
██╔═══╝ ██║     ██╔══██║  ╚██╔╝     ██║   ██╔══╝  ╚════██║   ██║
██║     ███████╗██║  ██║   ██║      ██║   ███████╗███████║   ██║
╚═╝     ╚══════╝╚═╝  ╚═╝   ╚═╝      ╚═╝   ╚══════╝╚══════╝   ╚═╝</pre>

<p>
  <img alt="Version: 0.1.0" src="https://img.shields.io/badge/version-0.1.0-blue.svg?cacheSeconds=2592000" />
  <a href="https://github.com/Archont561/playtest/blob/main/docs/README.md" target="_blank">
    <img alt="Documentation" src="https://img.shields.io/badge/documentation-yes-brightgreen.svg" />
  </a>
  <img alt="rust 1.88+" src="https://img.shields.io/badge/rust-%3E%3D1.88-blue.svg" />
  <img alt="python 3.12+" src="https://img.shields.io/badge/python-%3E%3D3.12-blue.svg" />
  <a href="https://codecov.io/gh/Archont561/playtest" target="_blank">
    <img alt="Coverage" src="https://img.shields.io/codecov/c/github/Archont561/playtest.svg" />
  </a>
  <a href="https://github.com/Archont561/playtest/blob/main/LICENSE" target="_blank">
    <img alt="License: MIT" src="https://img.shields.io/github/license/Archont561/playtest" />
  </a>
</p>

> Universal testing framework across nine language ecosystems.
>
> One shared behavioral contract for dependency-injected fixtures, failure-safe
> LIFO teardown, code-first BDD pipelines, and property-based testing, delivered
> through idiomatic native SDKs and a Rust CLI.

### 🏠 [Homepage](https://github.com/Archont561/playtest#readme)

## Documentation

The [`docs/`](docs/README.md) and [`backlog/docs/`](backlog/docs/README.md)
directories are the source of truth for using and working on this repository:

- [Documentation index](docs/README.md): everything, organized by purpose
- [Development setup](docs/guides/development-setup.md): Pixi environments, airgapped sandbox restore, hooks, and task runners
- [Clean Code rule register](docs/concepts/rule-register.md): Alint, Semgrep, ast-grep, Vale, and Rude enforcement rules
- [Workspace crates and CLI distribution](backlog/docs/specs/0012-workspace-crates-and-cli-distribution.md): the five-crate Rust workspace, embedded SDKs, and binary channels
- [Workspace configuration and SDK mirrors](backlog/docs/specs/0013-workspace-configuration-and-sdk-mirrors.md): `playtest.toml` version pinning and committed `.playtest/` mirrors
- [Report protocol and runner orchestration](backlog/docs/specs/0014-report-protocol-and-runner-orchestration.md): native runner execution and structured JSONL test reporting
- [Polyglot parity contract](backlog/docs/specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md): shared fixture, LIFO teardown, BDD, and property-testing semantics
- [Architecture Decision Records](backlog/docs/adr/README.md): design decisions from ADR 0001 through ADR 0019
- [Milestone roadmap](backlog/docs/milestones/README.md): current version-only scaffold status and planned delivery from `v0.1.0` to `v1.1.0`
- [Agent directives](AGENTS.md): repository invariants, registry configuration, and automation boundaries
- [Contributing](CONTRIBUTING.md): setup, the checks CI runs, Conventional Commits, and merge policy

Planned work is tracked in [`backlog/`](backlog/tasks.yaml).

## Language Targets

Parity is a roadmap contract rather than a claim that every capability ships in
every language today: the repository currently holds the `0.1.0` version-only
workspace scaffold while [Milestone M1](backlog/docs/milestones/M1-v0.1.0-foundation.md)
delivers the `playtest` CLI binary, `init`, `sync`, and multi-channel packaging.

| Language | Ecosystem | Directory | Package / Artifact |
| :--- | :--- | :--- | :--- |
| **Rust** | Cargo / crates.io | `crates/` | `playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, `playtest-cli` |
| **Python** | PyPI / pytest | `python/` | `playtest-py` |
| **TypeScript** | npm / Bun | `typescript/` | `@archont561/playtest` |
| **C++** | CMake / Conan 2 | `cpp/` | `playtest-cpp` |
| **C** | CMake / Conan 2 | `c/` | `playtest-c` |
| **C#** | .NET / NuGet | `csharp/` | `Playtest` |
| **Go** | Go Modules / `pkg.go.dev` | `go/` | `github.com/Archont561/playtest/go` |
| **Java** | Maven Central | `java/` | `io.github.archont561:playtest` |
| **R** | CRAN / R-universe | `r/` | `playtest` |

## Install

Bootstrap the locked developer toolchain from `pixi.lock`, or materialize the
airgapped Linux sandbox offline:

```sh
pixi install --frozen
pixi run --frozen bun-install
pixi run install-hooks
```

Offline Linux environments can restore the packed `default` environment and all
vendored Cargo crates without network access via `./scripts/restore.sh`.

## Usage

```sh
pixi run --frozen -- cargo run -p playtest-cli --locked
```

## Run tests

```sh
pixi run test-all
```

Run the fast governance, Clean Code, workflow, TOML, sandbox, and automation
suite with:

```sh
pixi run --frozen doit fast
```

## Author

👤 **Playtest contributors**

- Website: [archont561.github.io/Archont561](https://archont561.github.io/Archont561/)
- GitHub: [@Archont561](https://github.com/Archont561)

## 🤝 Contributing

Contributions, issues and feature requests are welcome!<br />
Feel free to check the [issues page](https://github.com/Archont561/playtest/issues). You can also take a look at the
[contributing guide](CONTRIBUTING.md): it covers setup, the checks a change has
to pass and the merge policy.

## Show your support

Give a ⭐️ if this project helped you!

## 📝 License

Copyright © 2026 [Playtest contributors](https://github.com/Archont561).<br />
This project is [MIT](https://github.com/Archont561/playtest/blob/main/LICENSE) licensed.

***

*This README was generated with ❤️ by [readme-md-generator](https://github.com/kefranabg/readme-md-generator) from [`automation/readme/README.ejs`](automation/readme/README.ejs).*
