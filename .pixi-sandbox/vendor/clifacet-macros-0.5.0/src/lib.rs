//! Procedural macros for Clifacet.
//!
//! [`Facet`] is the companion derive decision-2 proposes: it connects a clap
//! command struct to typed input without introducing a second grammar. [`command`]
//! is the same pairing in one attribute, for callers who would rather not repeat
//! two derives that have to agree. `version!` reports this crate's package version.
//!
//! `Facet` is re-exported from the `clifacet` library, so applications depend on
//! that crate alone.

mod command;
mod commands;
mod facet;
mod field;
mod function;
mod input_type;
mod planned;
mod rules;
mod subcommands;

use proc_macro::{Literal, TokenStream, TokenTree};

/// Wraps a struct in clap's parser and the [`Facet`] bridge, in one attribute.
///
/// ```ignore
/// #[clifacet::command(input = Greeting)]
/// #[command(name = "greet", about = "Say hello")]
/// struct Cli { #[arg(long)] name: Option<String> }
/// ```
///
/// Expands to the item exactly as written, preceded by
/// `#[derive(::clap::Parser, ::clifacet::Facet)]` and `#[facet(…)]`. It takes the same
/// keys as `#[facet(…)]` — `input`, `bridge`, `check`, `collect` — and exists because a derive cannot add
/// another derive to the same item.
///
/// `clap` still has to be a direct dependency of the calling crate: `clap`'s derive
/// expands to code naming `clap::…`, which an attribute cannot redirect. `clifacet`
/// re-exports `clap`, so the version is the one this macro was built against.
///
/// # Panics
///
/// Panics only if Rust cannot parse fixed, compiler-generated tokens.
#[proc_macro_attribute]
pub fn command(attribute: TokenStream, item: TokenStream) -> TokenStream {
    command::expand(&attribute.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares a command tree: clap's subcommand enum, and the conversion, collection and
/// dispatch that go with it.
///
/// ```ignore
/// #[clifacet::subcommands(input = Request, output = Report)]
/// enum Commands { Add(Add), Remove(Remove) }
///
/// enum Request { Add(AddInput), Remove(RemoveInput) }
/// ```
///
/// Each variant is `Name(Leaf)`, where `Leaf` is a `#[clifacet::command(input = …)]`
/// struct; inline fields and unit variants are compile errors that say so. Expands to the
/// enum with `#[derive(::clap::Subcommand)]`, plus:
///
/// - `TryFrom<Tree> for Request`: the chosen leaf validated by its own `Facet`, wrapped in
///   the `Request` variant of the same name — so a missing, extra or mistyped `Request`
///   variant fails to compile;
/// - `clifacet::prompt::Collect`: the chosen leaf asks its own questions;
/// - with `output = Type`, `clifacet::Command`: the chosen leaf runs, its output converted
///   with `Into`;
/// - with `planned` as well, `clifacet::command::Planned` instead, delegating `plan`, so
///   `Command` comes from the library's blanket impl.
///
/// On a variant, `#[clifacet(name = "…", alias = "…", visible_alias = "…", hide)]` become
/// clap's `#[command(..)]`; a raw `#[command(..)]` passes through. A name or alias used
/// twice, or an unknown key, is a compile error naming it. A struct field holding the
/// tree is declared `#[clifacet(subcommands)]`, as the tree or an `Option` of it.
///
/// # Panics
///
/// Panics only if Rust cannot parse fixed, compiler-generated tokens.
#[proc_macro_attribute]
pub fn subcommands(attribute: TokenStream, item: TokenStream) -> TokenStream {
    subcommands::expand(attribute.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Adds the `Handler<Cx>` a `PlannedWith<Cx>` impl implies: `handle` executes the plan
/// and `preview` returns it, so a dry run and a real run share one path. See
/// `clifacet::command::PlannedWith`.
#[proc_macro_attribute]
pub fn planned(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(item);
    planned::expand(&attribute.into(), item.clone())
        .unwrap_or_else(|error| {
            // The item is kept, so the only error is the one that says what to write.
            let mut tokens = error.into_compile_error();
            tokens.extend(item);
            tokens
        })
        .into()
}

/// Derives `clifacet::Facet`: bridges a clap command struct to typed input.
///
/// # Attributes
///
/// - `#[facet(input = Type)]` — required. The typed input the bridge builds: any type
///   the generated impls can name — a qualified path, a generic type, an alias, a
///   projection such as `<T as Trait>::Input`. A lifetime in it must be declared on the
///   command struct, whose generics every generated impl takes, or be `'static`. Without
///   a `bridge` it must name a struct, built as `Self { .. }`. `Self`, `_`, `impl Trait`
///   and `'_` are compile errors at their tokens.
/// - `#[facet(check = path)]` — optional. Called as `path(&input) -> Result<(),
///   Failure>` at the end of the generated `TryFrom`, after every field rule, for a
///   rule across fields.
/// - `#[facet(bridge = path)]` — optional. Called as `path(cli) -> Result<Type,
///   Failure>` instead of the generated `TryFrom`, for input the declarations cannot
///   describe. `check` beside it is a compile error: the bridge runs its own checks.
/// - `#[facet(collect = path)]` — optional. Called as `path(self, session) ->
///   Result<Self, Failure>` instead of the generated `Collect`, for questions the
///   declarations cannot describe. A field's `ask` or `confirm` beside it is a compile
///   error: the function asks every question.
///
/// On a field, `#[facet(..)]` says how that one value is asked for and normalised:
///
/// - `ask = "…"` / `confirm = "…"` — generated into `clifacet::prompt::Collect`: ask
///   for an `Option` no flag supplied, parsed by the argument's own clap parser; put a
///   set `bool` flag to the user.
/// - `fill = expr`, `trim`, `non_empty`, `secret` — generated into
///   `TryFrom<Self> for Type`, applied in that order. A declared `bridge` owns
///   normalization instead, so these beside it are a compile error.
/// - `skip` — the generated `TryFrom` leaves the field out of the input; clap still
///   parses it. It takes no rule, and a declared `bridge` makes it a compile error.
/// - `form` — the field is listed in `Facet::form_fields`, in declaration order, for a
///   generated TUI form. Like `ask`, it needs a `long` or `short` option.
/// - `subcommands` — the field holds a command tree declared with
///   `#[clifacet::subcommands]`, or an `Option` of one: the generated `TryFrom` converts
///   the chosen leaf, `Collect` lets it ask its questions. It takes no other key.
///
/// Without a `bridge`, the derive implements `TryFrom<Self> for Type` and
/// `validate` delegates to it; fields with no rule are copied by name.
///
/// # Panics
///
/// Panics only if Rust cannot parse fixed, compiler-generated tokens.
#[proc_macro_derive(Facet, attributes(facet))]
pub fn facet(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    facet::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Expands to the version of the `clifacet-macros` package as a string literal.
///
/// # Panics
///
/// Panics if Rust cannot parse a fixed, valid compiler-error expression.
///
/// # Examples
///
/// ```
/// assert!(!clifacet_macros::version!().is_empty());
/// ```
#[proc_macro]
pub fn version(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        return "compile_error!(\"version! takes no arguments\")"
            .parse()
            .expect("static compile_error tokens are valid");
    }

    TokenTree::Literal(Literal::string(env!("CARGO_PKG_VERSION"))).into()
}

#[cfg(test)]
mod tests;

/// Registers a folder of commands in one line (CLI-33).
///
/// ```ignore
/// clifacet::commands!(list, show, new);
/// ```
///
/// Expands to one `mod` and one `pub use` per module, a `Commands` tree, a `Request` with
/// one variant per leaf input, an `Input`, and a `Cli` root that flattens
/// `clifacet::Globals`. Each module holds a leaf whose struct is its stem in Pascal case
/// (`search_history` → `SearchHistory`, named `search-history` by clap); the list is the
/// whole registry, so nothing is discovered and nothing registers itself.
///
/// The keys after the module names — `globals = Type`, `output = Type`, `context = Type`,
/// `planned`, `name = "…"`, `about = "…"`, `version_off` — are documented with the macro
/// itself, in `clifacet::commands`. It is a `macro_rules!` there, so the expansion is
/// readable next to the page that explains it.
///
/// # Panics
///
/// Panics only if Rust cannot parse fixed, compiler-generated tokens.
#[doc(hidden)]
#[proc_macro]
pub fn __commands(input: TokenStream) -> TokenStream {
    // The call site, taken from the token stream itself: a `commands!()` with no list uses
    // it to find the folder it was written in. `macro_rules!` and proc-macro invocations
    // carry the caller's span, so this is the file the application wrote, not this crate.
    let tokens: proc_macro2::TokenStream = input.into();
    // The first token's span, not `Span::call_site()`: a span from inside a `macro_rules!`
    // points at this macro, while a token's span names the file the application wrote —
    // which is the folder discovery has to find.
    let call_site = tokens
        .clone()
        .into_iter()
        .next()
        .map_or_else(proc_macro2::Span::call_site, |token| token.span());
    commands::expand(tokens, call_site).into()
}

/// Registers a folder of commands, on the module that holds it (CLI-58).
///
/// ```ignore
/// // src/lib.rs: the whole registration
/// #[clifacet::registry]
/// mod commands {}
///
/// // src/commands/mod.rs: everything else about the folder, and no `commands!(..)` call
/// pub struct Request { .. }
/// ```
///
/// The braces are what make the declaration legal on every supported compiler: an *outlined*
/// module (`mod commands;`) in an attribute's input is unstable below Rust 1.99. An empty
/// body is filled from the folder's `mod.rs`; a body written here is the folder itself.
///
/// Expands to the folder's module — its own `mod.rs`, pulled in so its `mod` lines resolve
/// against its own directory — and beside it the same registration `clifacet::commands!(..)`
/// writes: `Commands`, `Request`, `Input`, `Cli` and the leaf re-exports. The module list
/// comes from the folder's build report, so this needs `clifacet::build::commands()` in the
/// application's `build.rs` and clifacet's `build` feature, exactly as `commands!()` with no
/// list does.
///
/// What the folder's types gain is that they are written in the folder's own scope and
/// re-exported into the registered module, so a `Request`, a helper, or an `impl` naming a
/// crate type goes on meaning what it did.
///
/// # Keys
///
/// The same as `clifacet::commands!(..)`, except the ones that describe the folder rather
/// than the registration: there is no module list (the folder is the list), no `root` and no
/// `request` (they belong beside the `Request` they describe), and no `context` — a context is
/// declared by the command that needs it, as a `cx: &Cx` parameter, so there is nothing for
/// this line to repeat.
///
/// - `output = Type`, `globals = Type`, `planned`, `version_off`, `name`, `about`,
///   `long_about` — as in `commands!(..)`;
/// - `path = "…"` — the folder, for an inline module whose name does not say where it is.
///
/// # Panics
///
/// Panics only if Rust cannot parse fixed, compiler-generated tokens.
#[proc_macro_attribute]
pub fn registry(attribute: TokenStream, item: TokenStream) -> TokenStream {
    commands::registry_expand(&attribute.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
