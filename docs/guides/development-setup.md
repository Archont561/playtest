# Development Setup

Playtest uses a single root `pixi.toml` manifest with isolated feature environments for Rust, repository automation, and each polyglot SDK target.

## Offline Sandbox Restore

In air-gapped or restricted environments, restore the pre-packed Linux developer environment and vendored Cargo registry from the `sandbox/developer-linux-64` branch:

```sh
./scripts/restore.sh
export PATH="$HOME/.local/bin:$PATH"
```

The restored bundle provides `pixi`, `pixi-sandbox`, Rust, Python, Bun, `doit`, Alint, Vale, ast-grep, Semgrep, `rude`, and all locked crates in `.pixi-sandbox/vendor`.

## Online Pixi Install

On a workstation with internet access, install the default developer environment or a specific SDK environment directly from `pixi.lock`:

```sh
pixi install --locked
pixi install --locked -e py
pixi install --locked -e ts
pixi install --locked -e go
```

## Running Checks Through `doit`

All multi-step repository automation lives in `automation/` behind `automation/dodo.py` (`doit.cfg`):

```sh
pixi run --frozen do list
pixi run --frozen do fast
pixi run --frozen do clean_code
pixi run --frozen do test_automation
```

Install local Lefthook hooks once per clone with `pixi run --frozen install-hooks`. Hooks execute with `CARGO_NET_OFFLINE=true` and `--no-install` so commits and pushes work in offline sandboxes.
