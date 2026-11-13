//! `clifacet::commands!(list, show, new)`: register a command folder in one line (CLI-33).
//!
//! A tree is declared where its leaves live (decision-14), but the registration around it
//! is mechanical: one `mod` line, one `pub use` and one enum variant per leaf, a second
//! variant in the request, and a root struct. This macro writes all of that from a list of
//! module names, so a folder holds exactly one registration line:
//!
//! ```ignore
//! clifacet::commands!(list, show, new);
//! ```
//!
//! Nothing is discovered: the list *is* the registry, checked by the compiler. Every name
//! it generates is fixed, so a reader knows what a folder contains without opening it:
//!
//! | Item | What it is |
//! | --- | --- |
//! | `mod <name>;` and `pub use <name>::<Name>;` | the leaf struct, which is its own command |
//! | `enum Commands` | the tree: one `Name(<name>::Name)` variant per module |
//! | `enum Request` | what the user asked for, validated: one variant per leaf input |
//! | `struct Input` | the root's validated input: `command: Option<Request>` |
//! | `struct Cli` | the root: the subcommand, [`Globals`](clifacet::Globals), the extras |
//!
//! The tree is the same `#[clifacet::subcommands]` this crate's attribute generates, so
//! dispatch stays compile-time and a request that leaves out a variant does not compile.
//! The file stem is the command name in kebab case and the struct is its Pascal case
//! form, as `clap` itself would name them — `search_history` is `SearchHistory` named
//! `search-history` — and a leaf may still rename itself with `#[command(name = "..")]`.
//!
//! Keys, after the module names:
//!
//! - `globals = Type`: the application's own global options, a `clap::Args` type,
//!   flattened beside `Globals` and kept out of the input with `#[clifacet(skip)]`;
//! - `output = Type`: what every leaf's output converts into, for a tree that runs;
//! - `context = Type`: the application's context, threaded to the chosen leaf (CLI-47);
//! - `planned`: the tree delegates `plan` instead of running;
//! - `name = "…"`, `about = "…"`: the root's name and description, otherwise taken from
//!   Cargo at compile time;
//! - `version_off`: no `--version`, for a root that forwards version queries elsewhere.
//!
//! # The same registration, on the module instead (CLI-58)
//!
//! `#[clifacet::registry(..)]` on `mod commands {}` writes all of the above from the folder's
//! report, and pulls the folder's `mod.rs` in beside it — so the folder holds its commands
//! and its own types, and the registration is one line in `src/lib.rs` (decision-21). This
//! is the mechanism it uses. An attribute cannot name a directory, and a declarative macro
//! cannot be reached under the name of an attribute macro, so the registration runs as a
//! proc macro; `include!` is what puts the folder's file back in its own directory, where its
//! nested `mod` lines still mean the files beside it.
//!
//! What changes for a caller, and why:
//!
//! - **The module is written `mod commands {}`, with empty braces.** An outlined module
//!   (`mod commands;`) in an attribute's input was unstable until Rust 1.99, and a proc macro
//!   cannot ask for a newer compiler than the crate's floor. The braces make the module
//!   inline, which every supported compiler accepts; the empty body is filled from the
//!   folder's file, and a body written here means the folder *is* this module.
//! - **No list.** The folder is the list, read from the same report `commands!()` reads, so a
//!   module name written as a key is refused with a message saying so.
//! - **No `context`.** A command that needs the application's context declares `cx: &Cx` on
//!   itself, so there is nothing for this line to repeat.
//! - **`Action`, not `Request`, by default.** The folder is where a hand-written `Request`
//!   lives; `request = Ident` names the generated one when an application wants otherwise.
//! - **The folder's `mod.rs` may not start with `//!` or `#![..]`**, because an included
//!   fragment may not carry inner attributes. It is checked first, and reported with the fix.
//! - **A helper is private.** Its `mod` line is written by the registry and never re-exported,
//!   so reaching one from outside the folder is `E0603`. The note for that error points at
//!   the attribute rather than at the folder, because a proc macro cannot place a span in
//!   another file — decision-21 records the trade.

use std::path::{Path, PathBuf};

use heck::ToUpperCamelCase as _;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, quote, quote_spanned};
use syn::parse::{ParseStream, Parser as _};
use syn::spanned::Spanned as _;
use syn::{Ident, Item, ItemMod, LitStr, Token, Type};

use crate::subcommands;

/// Where the report's fields live, as `clifacet::build` writes them.
///
/// Spelled here rather than imported, because `clifacet` depends on this crate: the macro
/// cannot name a type from the library, so the format is a contract between two crates that
/// each hold up their own end of it, checked on both sides (see `build_report.rs`).
mod protocol {
    /// The protocol this macro reads.
    pub(super) const PROTOCOL: &str = "clifacet-commands/1";
    /// The variable the report travels in.
    pub(super) const VARIABLE: &str = "CLIFACET_COMMANDS";

    /// The commands and groups a report names, in its own order, and where they live.
    pub(super) struct Report {
        pub(super) folder: std::path::PathBuf,
        pub(super) commands: Vec<String>,
        pub(super) groups: Vec<String>,
        /// Helpers too: `commands!(..)` declares only commands and groups (a helper is not a
        /// leaf), but `#[clifacet::registry]` also has to declare the helpers it may not
        /// re-export, because the folder's own `include!` no longer writes those `mod` lines.
        pub(super) helpers: Vec<String>,
    }

    impl Report {
        /// Every folder report in a value, in the build script's order.
        ///
        /// A `folder=` record opens a report and every record after it belongs to that one,
        /// which is how one environment variable carries a whole tree.
        pub(super) fn decode(value: &str) -> Result<Vec<Self>, String> {
            let mut records = value.split(';');
            let header = records.next().unwrap_or_default();
            if header != PROTOCOL {
                return Err(format!(
                    "`{VARIABLE}` does not start with `{PROTOCOL}`; it is written by \
                     `clifacet::build::commands(..)` in the application's `build.rs`, and read \
                     by `clifacet::commands!()`, so check that the build script calls it and \
                     that both use one clifacet version"
                ));
            }
            let mut reports: Vec<Self> = Vec::new();
            for record in records {
                let Some((key, value)) = record.split_once('=') else {
                    continue;
                };
                match key {
                    "folder" => reports.push(Self {
                        folder: std::path::PathBuf::from(value),
                        commands: Vec::new(),
                        groups: Vec::new(),
                        helpers: Vec::new(),
                    }),
                    "command" | "group" | "helper" => {
                        let Some(report) = reports.last_mut() else {
                            continue;
                        };
                        let names = match key {
                            "command" => &mut report.commands,
                            "group" => &mut report.groups,
                            _ => &mut report.helpers,
                        };
                        names.push(value.to_owned());
                    }
                    _ => {}
                }
            }
            Ok(reports)
        }
    }
}

use protocol::{Report, VARIABLE};

/// What the generated request is called when nothing names it.
///
/// `Action`, not `Request`: a folder registers itself so that it may hold its own `Request` —
/// the application's own type, with its own conversions — beside the generated one, which is
/// the list of variants. `commands!(..)` writes into a file that is registration only, so
/// `Request` is free there and stays the default for it.
const DEFAULT_REQUEST: &str = "Request";

/// Said when the call is in a file the compiler did not name to us.
const NO_LOCAL_FILE: &str = "`clifacet::commands!()` with no list finds its folder from the file \
                             the call is written in, and the compiler gave no file for this \
                             call; name the modules instead with `clifacet::commands!(list, ..)`";

/// Said when there is no report in the environment.
const NO_REPORT: &str = "`clifacet::commands!()` with no list reads the report its folder's \
                         `build.rs` wrote, and there is none: call \
                         `clifacet::build::commands()` from the build script, with clifacet's \
                         `build` feature enabled, or name the modules explicitly with \
                         `clifacet::commands!(list, show)`";

/// The keys `commands!(..)` takes once its module names are read, for messages.
const SUPPORTED_KEYS: &str = "`globals = Type`, `output = Type`, `context = Type`, `planned`, \
                              `name = \"…\"`, `about = \"…\"`, `long_about = \"…\"`, \
                              `version_off`, `root = { \"field\": Type }`, `request = Ident`";

/// One call: the modules it registers, and the keys it sets.
#[derive(Clone, Default)]
struct Declared {
    modules: Vec<syn::Path>,
    globals: Option<Type>,
    output: Option<Type>,
    context: Option<Type>,
    planned: bool,
    name: Option<LitStr>,
    about: Option<LitStr>,
    long_about: Option<LitStr>,
    version_off: bool,
    /// What the generated request enum is called, when the call names it. `None` leaves the
    /// default to whoever emits the registration: `Request` for `commands!(..)`, `Action` for
    /// `#[clifacet::registry]`, whose folder may hold a `Request` of its own.
    request: Option<Ident>,
    /// The root's own fields, declared the way a `#[clifacet::command]` struct declares
    /// them and folded into the generated input by name: `root = {"json": bool}`.
    root: Vec<(Ident, Type)>,
    /// Where the call was written, so a call with no list can find its own folder.
    call_site: Option<Span>,
    /// `path = "…"`: the folder, when the attribute names one rather than guessing it.
    path: Option<LitStr>,
    /// The folder's helper modules, when this declaration is a registry: helpers are declared
    /// with the leaves, and never registered in the tree.
    helpers: Vec<String>,
    /// The folder as the build report names it, when the caller already knows it.
    report_folder: Option<PathBuf>,
    /// The folder's absolute path, when a registry writes the leaves' `mod` lines outside the
    /// folder: a macro's `mod x;` resolves against the file the macro ran in, so each
    /// declaration says where its file is instead.
    in_folder: Option<PathBuf>,
    /// Whether the module list came from the folder's report rather than the call.
    discovered: bool,
}

impl Declared {
    /// This declaration with another's *keys* — the modules are the registry's own, and a
    /// duplicate name is the registry's to refuse, not something to copy over.
    fn merge(mut self, keys: Self) -> Self {
        self.globals = keys.globals.or(self.globals);
        self.output = keys.output.or(self.output);
        self.context = keys.context.or(self.context);
        self.planned = keys.planned || self.planned;
        self.name = keys.name.or(self.name);
        self.about = keys.about.or(self.about);
        self.long_about = keys.long_about.or(self.long_about);
        self.version_off = keys.version_off || self.version_off;
        self.in_folder = keys.in_folder.or(self.in_folder);
        // The report's helpers stand unless the keys brought their own: an attribute never
        // lists helpers, and the folder's report does.
        if !keys.helpers.is_empty() {
            self.helpers = keys.helpers;
        }
        if !keys.root.is_empty() {
            self.root = keys.root;
        }
        // Only when the keys named one: an entry that says nothing about the request must not
        // put a default back.
        self.request = keys.request.or(self.request);
        self
    }

    /// Rejects what a valid call cannot mean: no command, a command twice, `planned`
    /// with a context.
    fn finish(self) -> syn::Result<Self> {
        if self.modules.is_empty() {
            if self.discovered {
                // Discovery found no commands. That is a real state — a folder before its
                // first command — but registering nothing would generate an empty tree, so
                // it is refused with the one thing the caller can do about it.
                return Err(syn::Error::new(
                    self.call_site.unwrap_or_else(Span::call_site),
                    "`clifacet::commands!()` found no commands in this folder: add a `<name>.rs` \
                     beside `mod.rs`, or name the modules explicitly with \
                     `clifacet::commands!(list, show)`",
                ));
            }
            return Err(syn::Error::new(
                self.call_site.unwrap_or_else(Span::call_site),
                "`clifacet::commands!` needs at least one command: name the modules that hold \
                 them, `clifacet::commands!(list, show, new)` — one file per command, and this \
                 list is the only registry",
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for module in &self.modules {
            if !seen.insert(quote!(#module).to_string()) {
                return Err(syn::Error::new(
                    module.span(),
                    format!(
                        "`{}` is listed twice; one module, one command — drop the second",
                        quote!(#module)
                    ),
                ));
            }
        }
        if self.context.is_some() && self.planned {
            return Err(syn::Error::new(
                Span::call_site(),
                "`context` and `planned` cannot both be declared: with a context the tree \
                 previews through each leaf's own `Handler::preview`, so `planned` has \
                 nothing to add — drop it",
            ));
        }
        Ok(self)
    }

    /// The root's own fields as the generated input declares them: copied by name.
    fn root_fields(&self) -> Vec<TokenStream> {
        self.root
            .iter()
            .map(|(field, ty)| {
                quote! {
                    /// A field of the root, copied as declared.
                    pub #field: #ty,
                }
            })
            .collect()
    }

    /// The root's own flags: declared here, so the input copies them by name, and global,
    /// so a subcommand may be written before or after them.
    fn root_flags(&self) -> Vec<TokenStream> {
        self.root
            .iter()
            .map(|(field, ty)| {
                quote! {
                    /// A field of the root, declared by `commands!`: a global long flag, in
                    /// the input under the same name.
                    #[clifacet(long)]
                    #[arg(global = true)]
                    pub #field: #ty,
                }
            })
            .collect()
    }

    /// `long_about = "…"` for clap, or nothing.
    fn long_about_attr(&self) -> Option<TokenStream> {
        self.long_about
            .as_ref()
            .map(|long_about| quote!(long_about = #long_about,))
    }

    /// The leaf struct each module holds, named in Pascal case.
    ///
    /// A module may nest (`db::migrate`), and then the leaf is `Migrate` in the module of
    /// the same name: the path is the module's, the struct is its last segment's.
    fn leaves(&self) -> Vec<LeafPath> {
        self.modules
            .iter()
            .map(|module| {
                let last = module.segments.last().map_or_else(
                    || Ident::new("command", Span::call_site()),
                    |segment| segment.ident.clone(),
                );
                LeafPath {
                    path: module.clone(),
                    leaf: pascal(&last),
                    variant: pascal(&last),
                    name: last,
                }
            })
            .collect()
    }
}

/// One registered command: where it lives, its struct, and its variant in both enums.
struct LeafPath {
    path: syn::Path,
    leaf: Ident,
    variant: Ident,
    /// The module's own name, which a declaration outside the folder needs for its file.
    name: Ident,
}

/// `list` as `List`, `search_history` as `SearchHistory`.
fn pascal(module: &Ident) -> Ident {
    // `Span::call_site()` rather than the module's own span: the tree's generated arms
    // span their bindings at the leaf type, and a span from inside this expansion would
    // put those bindings in this macro's hygiene context instead of the caller's.
    Ident::new(&module.to_string().to_upper_camel_case(), Span::call_site())
}

/// Expands one `commands!(..)` call into the registration, or an error saying what to write.
pub(crate) fn expand(input: TokenStream, call_site: Span) -> TokenStream {
    match parse(input, call_site, None, None, None).and_then(|declared| tokens(&declared)) {
        Ok(tokens) => tokens,
        Err(error) => error.into_compile_error(),
    }
}

/// Like [`parse`], for a caller that has already said where the folder is.
fn parse_into(
    declared: Declared,
    file: Option<PathBuf>,
    folder: Option<PathBuf>,
    report: Option<&str>,
) -> syn::Result<Declared> {
    let mut parsed = parse(
        quote!(),
        declared.call_site.unwrap_or_else(Span::call_site),
        file,
        folder,
        report,
    )?;
    parsed.path = declared.path;
    parsed.in_folder = declared.in_folder;
    // Discovery's own answer, which the caller set and `parse` cannot know: the folder the
    // leaves are declared in and the name the report uses for it.
    parsed.report_folder = parsed.report_folder.or(declared.report_folder);
    parsed.call_site = declared.call_site.or(parsed.call_site);
    Ok(parsed)
}

/// Reads the module list and the keys after it.
///
/// `folder` is the file whose folder a call with no list registers. A `commands!(..)`
/// invocation has no file of its own — the macro only sees a span — so it passes `None` and
/// the span is asked instead. The `#[clifacet::registry]` attribute *is* written in the file,
/// so it says which one, which is also how the same shared reader serves both.
fn parse(
    input: TokenStream,
    call_site: Span,
    file: Option<PathBuf>,
    folder: Option<PathBuf>,
    report: Option<&str>,
) -> syn::Result<Declared> {
    let mut file = file;
    let mut folder = folder;
    let parser = |stream: ParseStream<'_>| -> syn::Result<Declared> {
        let mut declared = Declared {
            call_site: Some(call_site),
            ..Declared::default()
        };
        // One entry per comma-separated position, in the order written. A position that
        // *starts* with a comma is an empty entry — `output = X, , planned` — and skipping it
        // silently dropped whatever was meant there, which is how a bare `planned` between two
        // value keys used to disappear.
        let mut first = true;
        while !stream.is_empty() {
            if !first {
                stream.parse::<Token![,]>()?;
                if stream.is_empty() {
                    break;
                }
            }
            first = false;
            if stream.peek(Token![,]) {
                return Err(syn::Error::new(
                    stream.span(),
                    "an empty entry: `clifacet::commands!(..)` takes one declaration per \
                     position, so drop the extra comma",
                ));
            }
            declaration(stream, &mut declared)?;
        }
        discover(&mut declared, file.take(), folder.take(), report)?;
        declared.finish()
    };
    parser.parse2(input)
}

/// Everything the tree declares: the commands, then the groups, in the order they are known.
///
/// Helpers are not here. They are declared beside these but never become a variant, because
/// a helper is not a command — it is what the commands in the folder share (decision-19 item
/// 4), and `commands!` has nothing to register for it.
fn names(report: &Report) -> Vec<String> {
    report
        .commands
        .iter()
        .chain(&report.groups)
        .cloned()
        .collect()
}

/// Fills in the module list from the folder's report when the call named none.
///
/// `commands!()` with no list is discovery: the folder is the one the call was written in,
/// and `build.rs` has already scanned it and reported it through
/// [`VARIABLE`](clifacet::build::VARIABLE). Two facts make that possible without a proc
/// macro ever reading a directory (decision-15 item 4 stands):
///
/// - `Span::local_file()` gives the file the call is in, which is what the build script
///   named in its report (`src/commands` relative to the crate root, or the folder passed
///   to `commands_in`);
/// - the report says which of that folder's files are commands, which are groups and which
///   are helpers, so the macro does not have to guess from a name.
///
/// A call that named modules is left exactly as it was: moving between the two forms
/// changes only the call, which is what CLI-50's last acceptance criterion asks for.
fn discover(
    declared: &mut Declared,
    file: Option<PathBuf>,
    folder: Option<PathBuf>,
    report: Option<&str>,
) -> syn::Result<()> {
    // A call that names its modules needs nothing else.
    if !declared.modules.is_empty() {
        return Ok(());
    }
    // The report is checked first: "no build script" is the mistake a caller makes most
    // often, and it is the one whose fix is a line in another file.
    let call_site = declared.call_site.unwrap_or_else(Span::call_site);
    let from_environment;
    let value = if let Some(report) = report {
        report
    } else {
        from_environment =
            std::env::var(VARIABLE).map_err(|_| syn::Error::new(call_site, NO_REPORT))?;
        &from_environment
    };
    discover_from(declared, file, folder, value)
}

/// Discovery, given the report: what [`discover`] does once the environment has been read.
///
/// The two are split so a report can be tested by passing one. Writing it into
/// `CLIFACET_COMMANDS` instead would mean `set_var`, which is `unsafe` in edition 2024 and
/// cannot appear in this crate — and would let two tests race over one variable.
fn discover_from(
    declared: &mut Declared,
    file: Option<PathBuf>,
    folder: Option<PathBuf>,
    value: &str,
) -> syn::Result<()> {
    // The folder is absolute when a registry found it; the report names it from the crate root.
    let folder = folder.or_else(|| declared.report_folder.clone());
    if !declared.modules.is_empty() {
        return Ok(());
    }
    let call_site = declared.call_site.unwrap_or_else(Span::call_site);
    let reports = Report::decode(value).map_err(|error| syn::Error::new(call_site, error))?;
    // Which report is this call, and how many folders above its leaves it sits. A registry
    // module knows the folder outright, so it is matched as it is. A `commands!(..)` call
    // only has the file it was written in, and recovers its folder from the file the
    // compiler named — which is how its own position decides what it registers.
    let (report, depth) = if let Some(folder) = folder {
        let found = reports
            .iter()
            .find(|report| same_folder(&report.folder, &folder))
            .ok_or_else(|| {
                syn::Error::new(
                    call_site,
                    format!(
                        "`#[clifacet::registry]` registers {} but the build script scanned {}. \
                         A registry line registers the folder its module is in; give another \
                         folder with `path = \"…\"`, or call \
                         `clifacet::build::commands_in(..)` for it",
                        folder.display(),
                        scanned(&reports)
                    ),
                )
            })?;
        (found, 0)
    } else {
        let Some(file) = file.or_else(|| call_site.local_file()) else {
            return Err(syn::Error::new(call_site, NO_LOCAL_FILE));
        };
        // The deepest folder wins: a file in `src/commands/db/` is inside `src/commands` too,
        // and taking the first match would register the group's commands in the folder above.
        reports
            .iter()
            .filter_map(|report| report_folder(&file, &report.folder).map(|depth| (report, depth)))
            .max_by_key(|(report, _)| report.folder.as_os_str().len())
            .ok_or_else(|| {
                syn::Error::new(
                    call_site,
                    format!(
                        "`clifacet::commands!()` was written in {} but the build script \
                         scanned {}. A call with no list registers the folder it is written \
                         in; name the modules instead with `clifacet::commands!(..)` to \
                         register another one",
                        file.display(),
                        scanned(&reports)
                    ),
                )
            })?
    };
    let registered = names(report);
    if registered.is_empty() {
        // A folder with nothing to register is a real state — a folder before its first
        // command — but registering nothing would generate an empty tree, so it is refused
        // with the one thing the caller can do about it.
        return Err(syn::Error::new(
            call_site,
            "`clifacet::commands!()` found no commands in this folder: add a `<name>.rs` \
             beside `mod.rs`, or name the modules explicitly with \
             `clifacet::commands!(list, show)`",
        ));
    }
    // A helper is declared where the leaves are, so it is named plainly: the prefix discovery
    // computes is for a `commands!` call written above the leaves, and the registry writes its
    // declarations inside the folder.
    let prefix = if declared.in_folder.is_some() {
        String::new()
    } else {
        "super::".repeat(depth)
    };
    declared.helpers = report
        .helpers
        .iter()
        .map(|name| format!("{prefix}{name}"))
        .collect();
    declared.modules = registered
        .iter()
        .filter_map(|name| syn::parse_str(&format!("{prefix}{name}")).ok())
        .collect();
    declared.discovered = true;
    Ok(())
}

/// Where a declared module's file is, by the rule Rust uses for `mod <name>;`.
fn leaf_file(folder: &Path, name: &Ident) -> String {
    let nested = folder.join(name.to_string());
    let file = if nested.join("mod.rs").is_file() {
        nested.join("mod.rs")
    } else {
        folder.join(format!("{name}.rs"))
    };
    file.display().to_string()
}

/// A declared helper's name.
///
/// The span is this macro's: a helper's `mod` line is written by the attribute, so an error
/// about reaching the helper from outside the folder points at the registration rather than
/// at the folder's own file. `commands!()` placed that error on `mod _shared;` because it
/// emitted that line from its own expansion; the registry cannot, and the trade is recorded
/// in decision-20 — the error still names the helper and says it is private, in the folder's
/// own vocabulary.
fn helper_name(helper: &str) -> Ident {
    let name = helper.rsplit("::").next().unwrap_or(helper);
    Ident::new(name, Span::call_site())
}

/// The folders a report carries, for a message that says what was scanned.
fn scanned(reports: &[Report]) -> String {
    if reports.is_empty() {
        return String::from("nothing");
    }
    reports
        .iter()
        .map(|report| report.folder.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether a reported folder is the one a registry module's file lives in.
///
/// The two are written from different roots — the report names a folder relative to the crate
/// (`src/commands`), while the compiler names an absolute path — so the comparison is a
/// suffix one, at a folder boundary so `x/src/commands` cannot answer for `src/commands`.
fn same_folder(reported: &Path, folder: &Path) -> bool {
    let reported = reported.to_str().unwrap_or_default().replace('\\', "/");
    let folder = folder.to_str().unwrap_or_default().replace('\\', "/");
    folder == reported || folder.ends_with(&format!("/{reported}"))
}

/// How deep inside `folder` the file is, or `None` when it is not inside it at all.
///
/// The two paths are written from different roots: the report names a folder relative to the
/// crate (`src/commands`), while the compiler names a file relative to the directory it was
/// invoked in — usually the workspace root, so `examples/x/src/commands/mod.rs`. Matching by
/// containment is what makes the two comparable without either side guessing the other's
/// root; the depth then comes from what follows the folder's text, because everything before
/// it says nothing about how many `super::`s a `mod` line needs.
fn report_folder(file: &std::path::Path, folder: &std::path::Path) -> Option<usize> {
    let file = file.to_str()?.replace('\\', "/");
    let folder = folder.to_str()?.replace('\\', "/");
    let (before, after) = file.split_once(&folder)?;
    // The prefix has to end at a boundary, or `xsrc/commands` would match `src/commands`.
    if !(before.is_empty() || before.ends_with('/')) {
        return None;
    }
    Some(folder_of_call(after))
}

/// How many folder levels below the folder the file itself sits.
///
/// `mod.rs` is the folder itself; anything deeper counts its own `super::`s. The count comes
/// from the part of the path after the folder's text, which the caller strips first:
/// `src/commands/db/mod.rs` after `src/commands` leaves `/db/mod.rs`, so one level.
fn folder_of_call(remainder: &str) -> usize {
    let Some((folders, _file)) = remainder.trim_start_matches('/').rsplit_once('/') else {
        return 0;
    };
    folders.matches('/').count() + 1
}

fn declaration(stream: ParseStream<'_>, declared: &mut Declared) -> syn::Result<()> {
    // A module name, or a nested path (`db::migrate`) that reaches into a group, unless
    // the first name is one this macro takes as a key.
    let first: Ident = stream.parse()?;
    let known = matches!(
        first.to_string().as_str(),
        "globals"
            | "output"
            | "context"
            | "planned"
            | "name"
            | "about"
            | "long_about"
            | "version_off"
            | "root"
            | "request"
    );
    if !known {
        let mut module: syn::Path = syn::Path::from(first.clone());
        while stream.peek(Token![::]) {
            stream.parse::<Token![::]>()?;
            let segment: Ident = stream.parse()?;
            module.segments.push(segment.into());
        }
        declared.modules.push(module);
        // A name that is no module is a key this macro does not take, and saying so now
        // beats "file not found for module".
        if stream.peek(Token![=]) {
            return Err(unknown_key(&first.to_string(), first.span()));
        }
        return Ok(());
    }
    let (name, key) = (first.to_string(), first);
    if stream.peek(Token![=]) {
        stream.parse::<Token![=]>()?;
        return match name.as_str() {
            "globals" => {
                declared.globals = Some(stream.parse()?);
                Ok(())
            }
            "output" => {
                declared.output = Some(stream.parse()?);
                Ok(())
            }
            "context" => {
                declared.context = Some(stream.parse()?);
                Ok(())
            }
            "name" => {
                declared.name = Some(stream.parse()?);
                Ok(())
            }
            "about" => {
                declared.about = Some(stream.parse()?);
                Ok(())
            }
            "long_about" => {
                declared.long_about = Some(stream.parse()?);
                Ok(())
            }
            "root" => {
                declared.root = root_fields(stream)?;
                Ok(())
            }
            "request" => {
                declared.request = stream.parse()?;
                Ok(())
            }
            known @ ("planned" | "version_off") => Err(syn::Error::new(
                key.span(),
                format!("`{known}` takes no value: write `{known}` on its own"),
            )),
            _ => Err(unknown_key(&name, key.span())),
        };
    }
    match name.as_str() {
        "planned" => declared.planned = true,
        "version_off" => declared.version_off = true,
        _ => {
            return Err(syn::Error::new(
                key.span(),
                format!("`{name}` takes a value, for example `{name} = MyType`"),
            ));
        }
    }
    Ok(())
}

/// Reads `{ "json": bool, "verbose": Option<String> }` into field declarations.
///
/// Only what the generated input copies can be declared here — a name, its type, and
/// either `#[clifacet(skip)]`-like absence or presence in the input. Anything else about
/// the root's own flags is written on the field this returns, in the root's own idiom.
fn root_fields(stream: ParseStream<'_>) -> syn::Result<Vec<(Ident, Type)>> {
    if !stream.peek(syn::token::Brace) {
        let found: TokenStream = stream.parse()?;
        return Err(syn::Error::new_spanned(
            found,
            "`root` takes the root's own input fields in braces, for example \
             `root = { \"json\": bool }` — a name for the input, its type for the value",
        ));
    }
    let content;
    syn::braced!(content in stream);
    let fields: syn::punctuated::Punctuated<RootField, Token![,]> =
        content.parse_terminated(|stream: ParseStream<'_>| stream.parse(), Token![,])?;
    Ok(fields
        .into_iter()
        .map(|field| (field.name, field.ty))
        .collect())
}

/// One `"name": Type` pair inside `root = { .. }`.
struct RootField {
    name: Ident,
    ty: Type,
}

impl syn::parse::Parse for RootField {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let field: LitStr = input.parse()?;
        let name = Ident::new(&field.value(), field.span());
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        Ok(Self { name, ty })
    }
}

fn unknown_key(key: &str, span: Span) -> syn::Error {
    syn::Error::new(
        span,
        format!(
            "`{key}` is not a key of `clifacet::commands!(..)`; supported keys: {SUPPORTED_KEYS}"
        ),
    )
}

/// What the generated request enum is called.
///
/// `Request` unless the call names one, and `Action` for a registry — whose folder may hold a
/// `Request` of its own, the application's type with its own conversions, beside the generated
/// list of variants.
fn request_name(declared: &Declared) -> Ident {
    declared.request.clone().unwrap_or_else(|| {
        let fallback = if declared.in_folder.is_some() {
            "Action"
        } else {
            DEFAULT_REQUEST
        };
        Ident::new(fallback, Span::call_site())
    })
}

/// The registration: modules, re-exports, the tree, the request, the input and the root.
fn tokens(declared: &Declared) -> syn::Result<TokenStream> {
    let leaves = declared.leaves();
    let declarations = declarations(declared, &leaves);
    let variants = leaves.iter().map(|leaf| {
        let (variant, name) = (&leaf.variant, &leaf.leaf);
        quote!(#variant(#name))
    });
    let requests = leaves.iter().map(|leaf| {
        let (variant, path, name) = (&leaf.variant, &leaf.path, &leaf.leaf);
        quote!(#variant(<#path::#name as ::clifacet::Facet>::Input))
    });
    let reexports = reexports(&leaves);

    // The tree's attribute is the one `#[clifacet::subcommands]` parses, forwarded whole: this
    // macro never re-implements the tree, it only says what to declare.
    let request = request_name(declared);
    let mut attribute = quote!(input = #request);
    if let Some(output) = &declared.output {
        attribute.extend(quote!(, output = #output));
    }
    if declared.planned {
        attribute.extend(quote!(, planned));
    }
    if let Some(context) = &declared.context {
        attribute.extend(quote!(, context = #context));
    }
    // The tree is expanded here, with the attribute's own parser: forwarding the attribute as
    // tokens would expand it in the attribute's context, where the leaf types this macro named
    // are invisible ("cannot find value `leaf`").
    let tree = subcommands::expand(
        attribute,
        quote! {
            #[derive(Clone)]
            pub enum Commands {
                #(#variants,)*
            }
        },
    )?;

    let root_fields = declared.root_fields();
    let root_flags = declared.root_flags();

    let name = declared.name.as_ref().map_or_else(
        || quote!(::core::env!("CARGO_PKG_NAME")),
        |name| quote!(#name),
    );
    let about = declared.about.as_ref().map_or_else(
        || quote!(::core::env!("CARGO_PKG_DESCRIPTION")),
        |about| quote!(#about),
    );
    let long_about = declared.long_about_attr();
    let version = if declared.version_off {
        quote!()
    } else {
        quote!(version,)
    };
    let globals = declared.globals.as_ref().map(|ty| {
        quote! {
            /// The application's own global options.
            #[command(flatten)]
            #[clifacet(skip)]
            pub globals_of_the_application: #ty,
        }
    });

    // The root runs what its tree runs: the same kind, delegating to the chosen leaf, so
    // an application's `main` hands the runner one command and a dry run reaches the leaf.
    let root_run = root(declared);

    Ok(quote! {
        #declarations
        #reexports

        /// What the user asked for, validated: one variant per command.
        // Prints and clones; equality is the payload's to offer, so it is not derived here.
        #[derive(Debug, Clone)]
        pub enum #request {
            #(#requests,)*
        }

        /// The tree: one variant per command, parsed by clap and dispatched by the compiler.
        #tree

        /// The root's validated input: what to run, and the root's own fields beside it.
        #[derive(Debug, Clone)]
        pub struct Input {
            /// What to run; none means the application's own default.
            pub command: Option<#request>,
            #(#root_fields)*
        }



        /// The root command: the subcommand, the globals every application gets, and the
        /// application's own.
        // `Clone`, because the runner parses once and may run the command again: the
        // root holds its leaves, so they are cloned with it.
        // `Clone`, because a run parses once and may run again; nothing else, because the
        // root holds the leaves and a leaf is whatever it needs to be.
        #[clifacet::command(input = Input)]
        #[derive(Clone)]
        #[command(name = #name, #version about = #about, #long_about)]
        pub struct Cli {
            /// The subcommand; none means the application's own default.
            // Spelled `Option`, not `::core::option::Option`: clap's derive recognizes
            // an optional subcommand by that exact path, and treats any other as required.
            #[clifacet(subcommands)]
            pub command: Option<Commands>,

            #[command(flatten)]
            #[clifacet(skip)]
            pub globals: ::clifacet::Globals,

            #(#root_flags)*

            #globals
        }

        #root_run
    })
}

/// The leaf modules: declared where the leaves are, or each with the file it means.
///
/// `commands!(..)` writes `mod x;` in the folder, which is where the leaves are. A registry
/// writes it from outside the folder, where that keyword would look beside the attribute, so
/// each declaration carries its own `#[path]` — still the compiled file, so an edit inside the
/// folder rebuilds the crate.
fn declarations(declared: &Declared, leaves: &[LeafPath]) -> TokenStream {
    let modules = &declared.modules;
    let helpers = helper_modules(declared);
    let Some(folder) = &declared.in_folder else {
        return quote!(#(mod #modules;)* #helpers);
    };
    let leaves = leaves.iter().map(|leaf| {
        let file = leaf_file(folder, &leaf.name);
        let name = &leaf.name;
        quote!(#[path = #file] mod #name;)
    });
    let helpers = declared.helpers.iter().map(|helper| {
        let name = helper_name(helper);
        let file = leaf_file(folder, &name);
        quote!(#[path = #file] #[allow(unused_imports)] mod #name;)
    });
    quote!(#(#leaves)* #(#helpers)*)
}

/// A helper is the folder's own business: declared, never re-exported.
fn helper_modules(declared: &Declared) -> TokenStream {
    let helpers = declared.helpers.iter().map(|helper| {
        let name = helper_name(helper);
        quote!(#[allow(unused_imports)] mod #name;)
    });
    quote!(#(#helpers)*)
}

/// The leaf re-exports, each spanned at the module it names.
///
/// A file without the struct it is supposed to hold then reports at that file rather than at
/// the call above it, which is what makes "a command file without its struct" a mistake in the
/// caller's own source (CLI-50, AC#4).
fn reexports(leaves: &[LeafPath]) -> TokenStream {
    let reexports = leaves.iter().map(|leaf| {
        let (path, name) = (&leaf.path, &leaf.leaf);
        let at = leaf
            .path
            .segments
            .last()
            .map_or_else(Span::call_site, |segment| segment.ident.span());
        quote_spanned!(at=> pub(crate) use #path::#name;)
    });
    quote!(#(#reexports)*)
}

/// The root's dispatch: it runs what its tree runs, so `main` hands the runner one command.
///
/// The bodies are written out rather than delegated to a helper trait: a root with a
/// context has a context to pass, and one without has none, and the two signatures differ
/// in exactly that.
fn root(declared: &Declared) -> TokenStream {
    let fallback = quote! {
        ::core::result::Result::Err(::clifacet::Failure::validation("name the command to run"))
    };
    if declared.planned {
        // `planned` is also a `Command` (the library's blanket impl), so it is checked
        // first: delegating `plan` is what makes a dry run reach the leaf.
        return quote! {
            impl ::clifacet::command::Planned for Cli {
                type Input = Input;
                type Output = <Commands as ::clifacet::command::Planned>::Output;

                fn plan(
                    &self,
                    input: Input,
                ) -> ::core::result::Result<
                    ::clifacet::command::Plan<'_, Self::Output>,
                    ::clifacet::Failure,
                > {
                    match (&self.command, input.command) {
                        (Some(tree), Some(request)) => {
                            <Commands as ::clifacet::command::Planned>::plan(tree, request)
                        }
                        _ => #fallback,
                    }
                }
            }
        };
    }
    match (&declared.output, &declared.context) {
        (_, Some(context)) => quote! {
            impl ::clifacet::Handler<#context> for Cli {
                type Input = Input;
                type Output = <Commands as ::clifacet::Handler<#context>>::Output;

                fn handle(
                    &self,
                    input: Input,
                    cx: &#context,
                ) -> ::core::result::Result<Self::Output, ::clifacet::Failure> {
                    match (&self.command, input.command) {
                        (Some(tree), Some(request)) => {
                            <Commands as ::clifacet::Handler<#context>>::handle(tree, request, cx)
                        }
                        _ => #fallback,
                    }
                }

                fn preview<'__clifacet>(
                    &'__clifacet self,
                    input: Input,
                    cx: &'__clifacet #context,
                ) -> ::core::result::Result<
                    ::clifacet::command::Plan<'__clifacet, Self::Output>,
                    ::clifacet::Failure,
                > {
                    match (&self.command, input.command) {
                        (Some(tree), Some(request)) => <Commands as ::clifacet::Handler<
                            #context,
                        >>::preview(tree, request, cx),
                        _ => #fallback,
                    }
                }
            }
        },
        (Some(output), None) => quote! {
            impl ::clifacet::Command for Cli {
                type Input = Input;
                type Output = #output;

                fn execute(
                    &self,
                    input: Input,
                ) -> ::core::result::Result<Self::Output, ::clifacet::Failure> {
                    match (&self.command, input.command) {
                        (Some(tree), Some(request)) => {
                            <Commands as ::clifacet::Command>::execute(tree, request)
                        }
                        _ => #fallback,
                    }
                }
            }
        },
        (None, None) => quote!(),
    }
}

// --- `#[clifacet::registry(..)]` on a module (CLI-58) --------------------------------

/// Reads `#[clifacet::registry(..)]`.
///
/// One entry per comma-separated key, read by the same `declaration` that reads
/// `clifacet::commands!(..)` — so one grammar, one set of messages, and no second dialect to
/// keep in step. Two things differ, both about the list: a module name is refused, because the
/// folder is the list, and `path = "…"` is accepted, because an inline module has to say where
/// its folder is.
fn registry_attribute(attribute: &TokenStream) -> syn::Result<Declared> {
    let mut declared = Declared::default();
    let parser = |stream: ParseStream<'_>, declared: &mut Declared| -> syn::Result<()> {
        let mut first = true;
        while !stream.is_empty() {
            if !first {
                stream.parse::<Token![,]>()?;
                if stream.is_empty() {
                    break;
                }
            }
            first = false;
            if !stream.peek(syn::Ident) {
                return Err(syn::Error::new(
                    stream.span(),
                    "`#[clifacet::registry(..)]` takes comma-separated keys, each one written \
                     `key` or `key = value`",
                ));
            }
            let key: Ident = stream.parse()?;
            if key == "path" {
                stream.parse::<Token![=]>()?;
                declared.path = Some(stream.parse()?);
                continue;
            }
            if !is_key(&key) {
                return Err(syn::Error::new(
                    key.span(),
                    format!(
                        "`{key}` is not a key of `#[clifacet::registry(..)]`, and a module name \
                         is not written here: the folder is the list, and every command file in \
                         it is registered from the build script's report"
                    ),
                ));
            }
            let mut entry = Declared::default();
            // `declaration` reads a key and what follows it; it needs the key to be first, and
            // it was already consumed to check that it is one.
            let rest = entry_of(stream, &key)?;
            declaration_from(rest, &mut entry)?;
            *declared = std::mem::take(declared).merge(entry);
        }
        Ok(())
    };
    let reader = |stream: ParseStream<'_>| parser(stream, &mut declared);
    syn::parse::Parser::parse2(reader, attribute.clone())?;
    Ok(declared)
}

/// Whether an identifier is a key `commands!(..)` — and so `registry(..)` — takes.
fn is_key(name: &Ident) -> bool {
    matches!(
        name.to_string().as_str(),
        "globals"
            | "output"
            | "context"
            | "planned"
            | "name"
            | "about"
            | "long_about"
            | "version_off"
            | "root"
            | "request"
    )
}

/// The entry `declaration` expects, reassembled from a key already read and what follows it.
fn entry_of(stream: ParseStream<'_>, key: &Ident) -> syn::Result<TokenStream> {
    let mut entry = key.to_token_stream();
    if stream.peek(Token![=]) {
        stream.parse::<Token![=]>()?;
        entry.extend(quote!(=));
        if stream.peek(syn::token::Brace) {
            let group: proc_macro2::Group = stream.parse()?;
            entry.extend(group.to_token_stream());
        } else {
            // A type for `output = MyType`, a literal for `name = "bl"`: both are read here,
            // and `declaration` decides which it expected.
            let value: proc_macro2::TokenStream = if stream.peek(syn::Lit) {
                let literal: syn::Lit = stream.parse()?;
                literal.to_token_stream()
            } else {
                stream.parse::<Type>()?.to_token_stream()
            };
            entry.extend(value);
        }
    }
    Ok(entry)
}

/// `declaration`, for an entry that is already a token stream.
fn declaration_from(tokens: TokenStream, declared: &mut Declared) -> syn::Result<()> {
    let parser = |stream: ParseStream<'_>| declaration(stream, declared);
    parser.parse2(tokens)
}

/// The folder the module registers, and the file whose `mod` lines reach it.
///
/// For `mod commands {}` in `src/lib.rs` that is `src/commands` and
/// `src/commands/mod.rs`: the
/// module's own file, which is what `local_file()` names, including under `#[path]`.
///
/// A module written inline — `#[clifacet::registry] mod commands { .. }` — has no file of its
/// own. Its folder is `src/commands` beside the file *it* is written in, which is where the
/// `commands` name points anyway; `path = "…"` says so explicitly for anything else.
fn locate(attribute: &Declared, module: &ItemMod) -> Located {
    // The file the declaration is written in, which is what a relative folder is relative to:
    // for `mod commands {}` that is the *parent*'s file — `lib.rs` or `mod.rs` — and the folder
    // is `commands` beside it.
    // Absolute, because the path is written into an `include!` that the compiler resolves
    // against the file the macro ran in — and a span names the file relative to wherever cargo
    // was invoked, which is not the same directory.
    let written = module
        .span()
        .local_file()
        .map(|file| std::fs::canonicalize(&file).unwrap_or(file));
    let root = std::env::var("CARGO_MANIFEST_DIR").ok().map(PathBuf::from);
    let relative = attribute.path.as_ref().map_or_else(
        || Path::new("src").join(module.ident.to_string()),
        |path| PathBuf::from(path.value()),
    );
    // The report names folders relative to the crate root, so the folder is known from the
    // manifest alone. The *file* has to be found where Rust finds it, which is relative to the
    // file the module is declared in.
    let folder = root
        .as_ref()
        .map_or_else(|| relative.clone(), |root| root.join(&relative));
    let file = match written.clone().filter(|_| attribute.path.is_none()) {
        // Inline: the module has no file, so the folder is beside this one.
        Some(written) if module.content.is_some() => {
            let parent = written.parent().unwrap_or(Path::new(""));
            parent_file(parent, &module.ident.to_string())
        }
        // Declared in a file: the folder is beside that file.
        Some(written) => written.parent().map_or_else(
            || folder.join("mod.rs"),
            |parent| parent_file(&parent.join(module.ident.to_string()), "mod"),
        ),
        None => folder.join("mod.rs"),
    };
    Located {
        file,
        folder: relative,
    }
}

/// `folder/mod.rs` when it exists, otherwise `folder.rs` — how Rust itself looks.
fn parent_file(folder: &Path, name: &str) -> PathBuf {
    let folder = if name == "mod" {
        folder.to_path_buf()
    } else {
        folder.join(name)
    };
    if folder.join("mod.rs").is_file() {
        folder.join("mod.rs")
    } else {
        folder.with_extension("rs")
    }
}

/// Where a registry module's contents are.
struct Located {
    /// The file whose `mod` lines declare the leaves.
    file: PathBuf,
    /// The folder it sits in, as the build report names it.
    folder: PathBuf,
}

/// Expands `#[clifacet::registry(..)] mod commands {}` into the registration *and* the folder.
///
/// The attribute owns the registration, so the folder writes no `commands!(..)` call and
/// declares no context: one line in `lib.rs` says what the commands are and what they are
/// given. `mod.rs` keeps everything else — its helpers, its `Request`, its `From` impls —
/// because it is still a module file, and still where its own types live.
///
/// Two mechanisms, because neither does the job alone. A macro sees tokens, not files, so it
/// cannot open the folder; and an attribute cannot name a directory (`#[path = "commands"]` is
/// refused: "is a directory"). So the folder's `mod.rs` is pulled in with `include!`, whose
/// `mod` lines resolve against the included file's own directory (decision-20), and the
/// registration is generated from the folder's build report — the one `commands!()` with no
/// list already reads (CLI-50).
pub(crate) fn registry_expand(
    attribute: &TokenStream,
    item: TokenStream,
) -> syn::Result<TokenStream> {
    let module: ItemMod = syn::parse2(item)?;
    let keys = registry_attribute(attribute)?;
    let located = locate(&keys, &module);
    expand_registry(&keys, &module, &located, None)
}

/// A registry's expansion with the folder's report given rather than read, for the tests.
///
/// The folder is the fixture's, and the `mod` line is the one an application writes, because
/// what is being tested is the expansion a user gets — not an arrangement only a test makes.
#[cfg(test)]
pub(crate) fn registry_for_test(
    attribute: &TokenStream,
    item: &TokenStream,
    folder: &str,
    report: &str,
) -> syn::Result<TokenStream> {
    let module: ItemMod = syn::parse2(item.clone())?;
    let keys = registry_attribute(attribute)?;
    // The folder as a path, which is what `locate` would have produced for `mod commands {}`
    // written beside it: the file the leaves are declared from, and the report's name for it.
    let located = Located {
        file: PathBuf::from(folder).join("mod.rs"),
        folder: PathBuf::from(folder),
    };
    expand_registry(&keys, &module, &located, Some(report))
}

/// The expansion both entry points share: the registration, and the folder it belongs to.
fn expand_registry(
    keys: &Declared,
    module: &ItemMod,
    located: &Located,
    report: Option<&str>,
) -> syn::Result<TokenStream> {
    let mut module = module.clone();
    // The file's header is checked before anything is read from it: a `mod.rs` that cannot be
    // included is not worth registering, and this is the one mistake whose fix is in the file
    // the attribute names.
    reject_inner_docs(&located.file, &module)?;
    if module.ident != "commands" {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "`#[clifacet::registry]` declares the folder's `mod` lines, `Cli`, `Commands`, \
             `Request` and `Input` in the module it is written on, so that module is the \
             folder and is named `commands`; for a module under another name, write its types \
             and call `clifacet::commands!(..)` inside it",
        ));
    }
    let folder = located.folder.clone();

    // The module list comes from the folder's report, read here because the attribute *is*
    // the call inside the folder: leaving a `commands!(..)` there too would register twice.
    // `folder` is the report's own name for the folder, so it also selects the entry, and the
    // declarations are written from inside the folder.
    let mut declared = Declared {
        path: keys.path.clone(),
        // The report's own name for the folder, which is what selects its entry — a name given
        // by `path = ".."` is relative to the crate, and this one is what the build script wrote.
        report_folder: Some(folder.clone()),
        // The leaves are beside the file being included, which is where the `#[path]`
        // declarations have to point.
        in_folder: located.file.parent().map(Path::to_path_buf),
        ..Declared::default()
    };
    declared.call_site = Some(module.ident.span());
    let mut declared = parse_into(declared, Some(located.file.clone()), Some(folder), report)?;
    declared = declared.merge(keys.clone());
    let registration = tokens(&declared)?;

    // Two cases, and they differ only in where the folder's contents come from.
    //
    // Written in place, the module *is* the folder: its items are declared where Rust would
    // have declared them, and the leaves are `mod` lines beside them. An empty body is the
    // other case — `mod commands {}` is how a folder is declared, because an *outlined*
    // module in an attribute's input is unstable below Rust 1.99 and this crate's floor is
    // 1.88 (decision-21). The braces make the module inline, which every supported compiler
    // accepts, and the folder's file is included into it.
    if let Some((_, items)) = module.content.take()
        && !items.is_empty()
    {
        let kept = items
            .iter()
            .filter(|item| !is_registration_call(item))
            .map(Item::to_token_stream)
            .collect::<Vec<_>>();
        let name = &module.ident;
        let visibility = &module.vis;
        let attrs = &module.attrs;
        return Ok(quote! {
            #(#attrs)*
            #visibility mod #name {
                #(#kept)*
                #registration
            }
        });
    }

    // The folder's own file: `mod.rs` is pulled in as it was written, and the registration
    // sits beside it. The leaves' own `mod` lines are the registration's, each with the
    // `#[path]` of its file, because a keyword a macro emits resolves against the file the
    // macro ran in rather than against the folder.
    //
    // Checked here rather than in `locate`, because this is the path that reads the file,
    // and an `include!` of nothing says less than a sentence does.
    if !located.file.is_file() {
        return Err(syn::Error::new_spanned(
            &module.ident,
            format!(
                "`#[clifacet::registry]` fills this module from {}, which does not exist. \
                 That file is the folder; create it, or say where the folder is with \
                 `path = \"…\"`",
                shown(&located.file)
            ),
        ));
    }
    let file = LitStr::new(&located.file.display().to_string(), module.ident.span());
    let name = &module.ident;
    let visibility = &module.vis;
    let attrs = &module.attrs;
    Ok(quote! {
        #(#attrs)*
        #visibility mod #name {
            #registration
            #[allow(unused_imports)]
            include!(#file);
        }
    })
}

/// A path as a message should name it: relative to the crate, with `/` separators.
///
/// An absolute path is long, machine-specific, and differs between platforms — which would
/// make every golden compile-fail case a per-platform file. `src/commands/mod.rs` says the
/// same thing to the reader and to the test.
fn shown(file: &Path) -> String {
    let relative = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok()
        .and_then(|root| file.strip_prefix(&root).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| file.to_path_buf());
    relative.to_string_lossy().replace('\\', "/")
}

/// Refuses a folder file whose header an `include!` cannot carry.
///
/// The file is read for this one check, and for nothing else: what is emitted comes from
/// `include!`, which the compiler tracks, so an edit inside the folder rebuilds the crate. A
/// proc macro reading the file's *contents* would not be tracked, which is why the registration
/// is included rather than pasted (decision-15 item 4 stays true for this reason).
fn reject_inner_docs(file: &Path, module: &ItemMod) -> syn::Result<()> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Ok(());
    };
    // Only the header can hold them: after the first item, Rust refuses `//!` itself.
    let inner = text
        .lines()
        .map(str::trim_start)
        .take_while(|line| {
            !line.starts_with("pub ") && !line.starts_with("mod ") && !line.starts_with("use ")
        })
        .any(|line| line.starts_with("//!") || line.starts_with("#!["));
    if inner {
        // The message names the folder as a folder rather than by its path: the path is long,
        // machine-specific, and would make this golden a per-platform file. Where the folder
        // is is already said by the module this is reported on.
        return Err(syn::Error::new_spanned(
            module,
            "the folder's `mod.rs` is included into this module, and a file pulled in with \
             `include!` may not start with `//!` or `#![..]`, because inner attributes are only \
             allowed at the top of a file Rust reads as a module. Move the folder's \
             documentation to this line as `///` comments, and keep `//` comments inside the \
             folder",
        ));
    }
    Ok(())
}

/// Whether an item is a `commands!(..)` call: the one thing the folder does not repeat.
fn is_registration_call(item: &Item) -> bool {
    let Item::Macro(call) = item else {
        return false;
    };
    call.mac
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "commands")
}
