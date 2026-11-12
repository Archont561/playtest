# Contributing to Playtest

Thank you for contributing to Playtest. All repository automation, dependency locks, and quality gates run inside hermetic Pixi environments.

## Prerequisites

- Git 2.40 or newer
- Either a restored offline developer bundle (`./scripts/restore.sh`) or Pixi 0.83.0 or newer (`pixi install --locked`)

## Local Setup

Clone the repository and bootstrap the locked developer environment and Git hooks:

```sh
git clone https://github.com/Archont561/playtest.git
cd playtest
./scripts/restore.sh
pixi run --frozen install-hooks
```

When network access to `conda-forge` and `crates.io` is available and you do not need the offline bundle, run `pixi install --locked` instead of `./scripts/restore.sh`.

## Quality Gates

Run the automation registry through `doit` (`automation/dodo.py`) before opening a pull request:

```sh
pixi run --frozen do fast
pixi run --frozen do clean_code
pixi run --frozen do test_automation
pixi run --frozen clippy
pixi run --frozen test
```

## Lockfile Hygiene

Pull requests are checked by `.github/workflows/relock.yml`. When a pull request edits `pixi.toml`, `Cargo.toml`, or any `crates/*/Cargo.toml` manifest without refreshing `pixi.lock` or `Cargo.lock`, `pixi-sandbox[bot]` resolves and commits the updated lockfiles onto the pull request branch.

## Conventional Commits

Every commit message must conform to `.versionrc`:

```sh
printf '%s\n' "feat(core): describe change" | pixi run --frozen lint-commit
```

## License

Contributions are licensed under the MIT license (`LICENSE`).
