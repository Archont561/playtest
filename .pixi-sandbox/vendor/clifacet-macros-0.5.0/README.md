<!--
  The README's source. `pixi run readme` renders it into README.md with
  readme-md-generator, and `pixi run readme-check` fails when the two disagree, so this
  file is what a change to the README edits — never the rendered one.

  Two kinds of line live here. The values in angle-bracket percent tags are read by
  the generator from
  `package.json` and from the repository's own git remotes, so the version, the URLs and
  the author are not typed twice. Everything else is prose, and it is the same prose the
  pages under `docs/` hold: this file introduces, the pages explain. Anything a page is
  better at saying is a link here.
-->
# clifacet

A Rust library for command-line applications whose commands are values: validated input
in, data or a failure out. One description of a command serves the flags, the prompts and
the full-screen form, so an application declares what a run may ask for once and the
frontend decides how to ask.

<p>
<a href="https://crates.io/crates/clifacet"><img src="https://img.shields.io/crates/v/clifacet?style=flat-square&label=crates.io" alt="crates.io"></a>
<a href="https://docs.rs/clifacet"><img src="https://img.shields.io/docsrs/clifacet?style=flat-square&label=docs.rs" alt="docs.rs"></a>
<a href="https://github.com/Archont561/clifacet/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Archont561/clifacet/ci.yml?branch=main&style=flat-square&label=CI" alt="CI status"></a>
<a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.88%20%E2%80%93%20current-orange?style=flat-square" alt="rust 1.88 - current"></a>
<a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=flat-square" alt="MIT OR Apache-2.0"></a>
</p>

## Why

A command is usually written three times: once as a `clap` parser, once as a set of
prompts for when the flags were left out, and once as a form for the terminal. The three
drift, and the drift is invisible until a user finds it. Clifacet asks for the description
once.

```rust
use clifacet::{Command, Failure};

struct Greet;

impl Command for Greet {
    type Input = String;
    type Output = String;

    fn execute(&self, input: Self::Input) -> Result<Self::Output, Failure> {
        Ok(format!("hello, {input}"))
    }
}
```

That is the seam: validated input in, data or a failure out, and no stream, terminal or
exit code in sight. Writing the parser and the validated input by hand is a second and
third description of the same command, so one attribute writes both — the `clap`
surface, the typed input, and the command that runs on it:

```rust
use clifacet::Facet;

#[clifacet::command(input = Greeting)]
#[command(name = "greet", about = "Say hello")]
struct Cli {
    /// Who to greet.
    #[arg(long, alias = "nickname")]
    name: Option<String>,
}

/// Validated input. Every value of it is runnable.
struct Greeting {
    name: Option<String>,
}
```

That expands to `#[derive(::clap::Parser, ::clifacet::Facet)]` plus
`#[facet(input = Greeting)]` in front of the item, so `#[derive(clap::Parser,
clifacet::Facet)]` on its own keeps working. `clap` remains a direct dependency, because
clap's derive emits code naming `clap::…`; this crate re-exports `clap` so the version is
the one the macro was built against.

The application keeps what it owns: which stream the data goes to, and which exit code a
failure costs.

## Install

```toml
[dependencies]
clifacet = "0.5.0"
```

Frontends are features, so an application pays only for what it uses: `interactive`
(prompts, with an `inquire` dialog), `tui` (full screen), and `serde` (failures and
redacted secrets that serialize, for an application that reports failures as JSON). See
[what each feature adds and costs](docs/guides/getting-started.md#features).

## Try it

```sh
cargo run -p clifacet-example                            # asks for what the flags left out
cargo run -p clifacet-example -- --name ada --repeat 2    # flags
cargo run -p clifacet-example -- --interactive            # a prompt, or a refusal
cargo run -p clifacet-example -- --tui                    # the full-screen form
```

[`examples/simple`examples/simple/README.md is that application, with its own README,
its sources and its suite.

## What is in the box

| Module | Answers |
| --- | --- |
| `clifacet::command` | what a command is: validated input in, data or a failure out |
| `clifacet::facet` | how a `clap` command becomes typed, validated input |
| `clifacet::policy` | which mode a run may use and which it prefers, resolved from a request, a policy and what the environment offers |
| `clifacet::prompt` | how a question is asked: a plain reader, an `inquire` dialog, or your own — and how to script the answers in a test |
| `clifacet::tui` | the full-screen form, its fields, and the two escape hatches for drawing your own |
| `clifacet::contract` | what a run may write where, and what each outcome costs |
| `clifacet::report` | writing a failure or `clap`'s verdict on the right stream, and answering with its exit code |
| `clifacet::harness` | proof that a different frontend changed nothing else |
| `clifacet::secret` | a credential that cannot be printed by accident |
| `clifacet::provenance` | where a value came from, recorded without recording the value |

Prompts are a seam that needs two methods — `ask` and `confirm`; choices come with them —
so the dialog is a default rather than a fixture: an application can hand `Session` its own
frontend and keep the policy, the streams and the exit codes.

## Documentation

`docs/` is the knowledge layer and it is checked, not remembered: every link resolves,
every ```` ```rust ```` fence is a compiled test, every documented `clifacet::…` path
names a public item, and the prose is linted for spelling and terminology.

| Read it when | Page |
| --- | --- |
| adding the crate and writing a first command | [Getting started](docs/guides/getting-started.md) |
| deciding which mode runs, and what a run costs | [Modes and the interaction policy](docs/concepts/modes.md) |
| giving an existing `clap` command validated input | [The companion derive](docs/guides/derive.md) |
| drawing your own field or screen | [TUI composition](docs/guides/tui-composition.md) |
| proving a promise without a terminal | [Testing](docs/guides/testing.md) |
| looking up a seam, a stream or an exit code | [Command core](docs/reference/command-core.md) |
| working on this repository | [Contributing](docs/development/contributing.md) |
| changing a page, or wondering what a change owes it | [The documentation policy](docs/concepts/documentation-policy.md) |

The [docs index](docs/README.md) is the map: `guides/` for tasks, `concepts/` for the
reasons, `reference/` for the promises, `architecture/` for how the repository is built,
and `development/` for working on it.

## Status

Experimental. The command core, the interaction policy, prompts, the derive, the `command`
wrapper, one `#[clifacet(..)]` declaration per field and the TUI are implemented and
covered; the JSON frontend is reserved and does not exist yet.

`0.5.0` is the version this README describes. Publishing is a separate,
authorized act from merging — a merge never puts a version on the registry by itself — see
the [changelogCHANGELOG.md for what each release changed.

## Workspace

| Crate | What it is |
| --- | --- |
| `clifacet` | the library: the command seam, the frontends, the interaction policy, the result contract |
| `clifacet-macros` | the `Facet` derive, the `#[clifacet::command]` wrapper and `version!`, re-exported through `clifacet` |
| [`clifacet-example`examples/simple/README.md | the reference application, its tests and runnable demos — a consumer of the published API, not published itself |
| `clifacet-discovered` | a small application whose folder registers itself from its `build.rs`: the smallest consumer that proves discovery |

## Develop

```sh
bash scripts/bootstrap.sh                   # checkout at the remote head, environment, hooks, smoke check
pixi run --frozen --no-install automation gates   # the offline gate, cheapest failure first
pixi run --frozen automation gates-online         # adds the checks that need the network
pixi run --frozen automation list                 # every task, with what it covers
```

`automation/` and `dodo.py` are the task graph; the scripts in `scripts/` remain runnable
on their own. `pixi run automation gates` runs in the same order CI does and covers
formatting, the linter, repository policy, the documentation gate, the changelog, the
feature matrix, the workflows, the sandbox launcher and every test suite.

Supported: the empty, `default`, `interactive`, `serde` and `tui` configurations plus
all-features together, on Linux, macOS and Windows, with Rust 1.88 as the floor — each one
checked in CI rather than assumed. See [supported platforms and feature
combinations](docs/development/development.md#supported-platforms-and-feature-combinations).

## Contributing

The [contributing page](docs/development/contributing.md) has the checks, the hooks and the layout. A
change is done when the gate passes, the documentation it affects is updated or the review
records why it was not, and every command in the prose was run.

## License

Licensed under either of [Apache License, Version 2.0LICENSE-APACHE or
[MIT licenseLICENSE-MIT at your option.

Archont561 — `https://github.com/Archont561/clifacet`
