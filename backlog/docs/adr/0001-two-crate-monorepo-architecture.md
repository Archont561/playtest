# ADR 0001: Two-Crate Monorepo Architecture

## Context
In Rust, procedural macros (e.g. `#[proc_macro_attribute]`) must reside in a crate declared with `proc-macro = true` in `Cargo.toml`. Such crates are legally prohibited from exporting normal structs, traits, or runtime constants. At the same time, consumers should not have to depend on multiple fragmented packages.

## Decision
Adopt the universal standard monorepo pattern (`tokio`/`tokio-macros`, `serde`/`serde_derive`):
- `crates/playtest`: The primary facade crate that end users install.
- `crates/playtest-macros`: The procedural macro crate, re-exported transparently by `playtest`.
- Repository-wide automation is maintained via Python automation scripts (`scripts/*.py`).

## Consequences
- Consumers only declare `playtest = "..."` in `Cargo.toml`.
- Procedural macros can be iterated on without breaking public API facades.
