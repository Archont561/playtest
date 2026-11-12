# Clean Code Rule Register

Python Clean Code is enforced twice: `automation/rude_clean.py` runs `rude check --format json` with `rude.toml` and fails on any `error`, `warning`, or `info` diagnostic, and the same command is wired into `pixi run lint`, `pixi run lint-automation`, the `py` lane `sdk-lint`, the `pre-commit` hook, and `pixi run do fast`.

Playtest vendors the curated native presets from `Archont561/polyglot-clean-code` under `vendor/clean-code-presets/` (`UPSTREAM.pin`), guarded against local edits by `vendor/clean-code-presets/VENDOR.sha256` and enforced by `pixi run --frozen do clean_code`.

## Active Preset Tiers

| Engine | Config Path | Scope | Canonical Rules |
| :--- | :--- | :--- | :--- |
| **Alint** | `vendor/clean-code-presets/alint/strict.yml` + `.alint.yml` | Repository structure, workflows, issue templates, MIT license, `scripts/restore.sh`-only boundary | `PCC-R01`–`PCC-R06` + Playtest file invariants |
| **Vale** | `vendor/clean-code-presets/vale/strict.ini` | `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `docs/` | `PCC-D01`–`PCC-D04` |
| **ast-grep** | `vendor/clean-code-presets/ast-grep/pedantic/sgconfig.yml` | `crates/`, `python/`, `typescript/`, `go/`, `java/` | `PCC-S01`–`PCC-S08` |
| **Semgrep** | `vendor/clean-code-presets/semgrep/pedantic.yml` | Repository-wide (excluding `vendor/clean-code-presets/`) | `PCC-P01`–`PCC-P08` |
| **Rude** | `rude.toml` (`rude-clean-code`) | `automation/**/*.py`, `python/src/**/*.py`, `python/tests/**/*.py` | `CC0`–`CC9` (all Clean Code families at strict defaults) |

## Updating Vendored Presets

Never edit files inside `vendor/clean-code-presets/` by hand. To adopt a newer upstream revision, copy `presets/` from `Archont561/polyglot-clean-code`, update `vendor/clean-code-presets/UPSTREAM.pin`, regenerate `vendor/clean-code-presets/VENDOR.sha256` via `python3 -m automation.clean_code --write-manifest`, and run `pixi run --frozen do clean_code`.

## Dependency Advisory Exception

`rstest-bdd 0.6.0` (used only in `[dev-dependencies]`) transitively pulls `basic-toml 0.1.10` (`RUSTSEC-2026-0374`, unmaintained). Both `deny.toml` and `.cargo/audit.toml` record a narrow ignore for `RUSTSEC-2026-0374`. `automation/advisory_exception.py` verifies this chain against `Cargo.lock` on every CI and daily security run and fails as soon as `basic-toml` leaves the dependency graph so the exception is removed promptly.
