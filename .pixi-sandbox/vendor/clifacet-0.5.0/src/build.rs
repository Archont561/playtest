//! `clifacet::build::commands()`: the report a build script sends its own crate (CLI-50).
//!
//! Discovery has to happen in the application's `build.rs`, because a proc macro may not
//! read the directory (decision-15 item 4). So the build script scans, and the macro reads
//! what it found — through an environment variable, because that is the only channel a
//! build script has to a macro in the same crate.
//!
//! The value is versioned. A `clifacet` newer than the `build.rs` was written against would
//! otherwise have to guess whether a missing field means "no commands" or "an older
//! protocol", and guessing is how a build silently loses commands:
//!
//! ```text
//! clifacet-commands/1;folder=src/commands;command=list;group=db;helper=_shared\
//! ;folder=src/commands/db;command=migrate
//! ```
//!
//! One `key=value` record per `;`, no quoting: a module name cannot contain `=` or `;`, so
//! the format needs no escaping. A `folder=` record begins a report, and every report after
//! it belongs to that folder — a group is a folder with its own `commands!`, so it needs its
//! own list, and the macro picks the one whose folder it is in. Records are separated by `;` rather than a newline because
//! `cargo:rustc-env` is one *line* of build-script output per directive — a value with a
//! newline in it is silently truncated at the first newline, which loses every record after
//! the header.

#![cfg(feature = "build")]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The protocol version this crate writes and the only one it reads.
pub const PROTOCOL: u32 = 1;

/// The environment variable the report travels in.
///
/// Named after the crate rather than the application, because the reader — `commands!`
/// inside `clifacet-macros` — has no way to know which application called it.
pub const VARIABLE: &str = "CLIFACET_COMMANDS";

/// What a folder holds, as the macro needs to know it.
///
/// Commands and groups are what `commands!` declares as modules of the tree; helpers are
/// the `_name.rs` files and `_name/` folders it declares privately. Ordering is stable —
/// sorted by name — so the generated code does not change when the filesystem's own order
/// does, which would rebuild an unchanged crate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The folder the report describes, relative to the crate root.
    pub folder: PathBuf,
    /// One module name per command, sorted.
    pub commands: Vec<String>,
    /// One module name per group, sorted.
    pub groups: Vec<String>,
    /// One module name per helper, sorted, with the leading `_` kept.
    pub helpers: Vec<String>,
    /// Every entry the scan looked at, so Cargo is told to watch exactly those.
    ///
    /// Not part of the protocol: it is what the build script needs to emit `rerun-if-changed`
    /// for, and the macro has no use for it.
    pub entries: Vec<String>,
}

impl Report {
    /// Reads a folder: one command per `<name>.rs`, one group or helper per directory.
    ///
    /// `mod.rs` is the folder's own file, not a command, and neither is anything without a
    /// `.rs` extension. A folder named `_name` is a helper and never a group.
    ///
    /// # Errors
    ///
    /// Returns the folder's own error when it cannot be read, naming the path: a build
    /// script that reports an empty folder where there is no folder is how a discovered
    /// command silently disappears.
    pub fn of(folder: impl AsRef<Path>) -> std::io::Result<Self> {
        let folder = folder.as_ref();
        let mut commands = BTreeSet::new();
        let mut groups = BTreeSet::new();
        let mut helpers = BTreeSet::new();
        for entry in std::fs::read_dir(folder)? {
            let entry = entry?;
            let path = entry.path();
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            // `mod.rs` is the folder's own module, and a name that is not an identifier is
            // not a module at all — an editor's backup file is the usual case.
            if stem == "mod" || !is_module_name(stem) {
                continue;
            }
            if path.is_dir() {
                let set = if stem.starts_with('_') {
                    &mut helpers
                } else {
                    &mut groups
                };
                set.insert(stem.to_owned());
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let set = if stem.starts_with('_') {
                    &mut helpers
                } else {
                    &mut commands
                };
                set.insert(stem.to_owned());
            }
        }
        let mut entries: Vec<String> = commands
            .iter()
            .chain(&groups)
            .chain(&helpers)
            .cloned()
            .collect();
        entries.sort();
        Ok(Self {
            folder: folder.to_path_buf(),
            commands: commands.into_iter().collect(),
            groups: groups.into_iter().collect(),
            helpers: helpers.into_iter().collect(),
            entries,
        })
    }

    /// One report as the environment variable carries it, without the header.
    ///
    /// A single report is encoded by [`Reports::encode`], which adds the protocol header and
    /// joins every folder of a tree; this is the part one folder contributes.
    fn records(&self) -> String {
        let mut records = format!("folder={}", self.folder.display());
        for kind in ["command", "group", "helper"] {
            for name in self.names(kind) {
                let _ = write!(records, ";{kind}={name}");
            }
        }
        records
    }

    /// The names of one kind, for [`records`](Self::records).
    fn names(&self, kind: &str) -> &[String] {
        match kind {
            "command" => &self.commands,
            "group" => &self.groups,
            _ => &self.helpers,
        }
    }
}

/// Every folder of one application's command tree, as the build script reported them.
///
/// More than one folder because a group is a folder with its own `commands!`, so a nested
/// tree needs a report per folder. The macro picks the report whose folder the call is in,
/// which is what lets a group's `mod.rs` hold a bare `commands!()` too.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reports {
    /// One report per folder, the scan's own order.
    pub folders: Vec<Report>,
}

impl Reports {
    /// The report as the environment variable carries it.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut encoded = format!("clifacet-commands/{PROTOCOL}");
        for report in &self.folders {
            let _ = write!(encoded, ";{}", report.records());
        }
        encoded
    }

    /// The reports a value carries, or the reason it is not one this crate can read.
    ///
    /// # Errors
    ///
    /// Returns a message naming the variable and what is wrong with it. The macro turns it
    /// into a compile error at the `commands!()` call: a build script from another `clifacet`
    /// version, or a hand-written value, is a mistake to fix rather than to guess at.
    pub fn decode(value: &str) -> Result<Self, String> {
        let mut records = value.split(';');
        let header = records.next().unwrap_or_default();
        let version = header
            .strip_prefix("clifacet-commands/")
            .and_then(|version| version.parse::<u32>().ok())
            .ok_or_else(|| {
                format!(
                    "`{VARIABLE}` does not start with `clifacet-commands/<version>`; it is written \
                     by `clifacet::build::commands(..)` and read by `clifacet::commands!()`, so \
                     check that `build.rs` calls it"
                )
            })?;
        if version != PROTOCOL {
            return Err(format!(
                "`{VARIABLE}` speaks protocol {version} and this clifacet speaks {PROTOCOL}; \
                 the build script's `clifacet` and the macro's are different versions — one \
                 `cargo update` for both fixes it"
            ));
        }
        let mut folders: Vec<Report> = Vec::new();
        for record in records {
            if record.is_empty() {
                continue;
            }
            let (key, value) = record.split_once('=').ok_or_else(|| {
                format!("`{VARIABLE}` has a record without `=`: `{record}`; it is not a report")
            })?;
            match key {
                // A `folder=` record opens the report every record after it belongs to.
                "folder" => folders.push(Report {
                    folder: PathBuf::from(value),
                    ..Report::default()
                }),
                "command" | "group" | "helper" => {
                    let Some(report) = folders.last_mut() else {
                        return Err(format!(
                            "`{VARIABLE}` has a `{key}=` record before any `folder=` record; a \
                             report starts with the folder it describes"
                        ));
                    };
                    match key {
                        "command" => report.commands.push(value.to_owned()),
                        "group" => report.groups.push(value.to_owned()),
                        _ => report.helpers.push(value.to_owned()),
                    }
                }
                other => {
                    return Err(format!(
                        "`{VARIABLE}` has a record this clifacet does not know: `{other}=…`; \
                         protocol {PROTOCOL} carries `folder`, `command`, `group` and `helper`"
                    ));
                }
            }
        }
        Ok(Self { folders })
    }

    /// The report for `folder`, when the build script scanned it.
    #[must_use]
    pub fn of<'a>(&'a self, folder: &str) -> Option<&'a Report> {
        self.folders
            .iter()
            .find(|report| report.folder.to_str() == Some(folder))
    }
}

impl Report {}

/// Whether `name` can be a Rust module: a letter or `_`, then letters, digits or `_`.
fn is_module_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_')
        && characters.all(|character| character.is_alphanumeric() || character == '_')
}

/// What a folder's report becomes in the source: the `mod` lines, and nothing else.
///
/// This is the whole of discovery on the macro side, and it is deliberately here rather
/// than in `clifacet-macros` so it can be tested as a value. The macro reads the variable,
/// calls this, and parses the result, so the text is checked against the compiler the same
/// way written source is — there is no separate model of a module tree to drift.
///
/// The visibility rules are decision-19 item 4's:
///
/// - a command and a group are `pub`, because `commands!` re-exports them;
/// - a helper is private to the folder, so `mod _shared;` — visible to `mod.rs` and to the
///   groups below it, which reach it as `super::_shared` or `super::super::_shared`;
/// - a helper is never re-exported, so reaching one from outside the folder is E0603 at the
///   caller's line rather than a silent part of the public surface.
#[must_use]
pub fn declarations(report: &Report) -> String {
    let mut declarations = String::new();
    for name in report.commands.iter().chain(&report.groups) {
        let _ = writeln!(declarations, "pub mod {name};");
    }
    for name in &report.helpers {
        // A helper's items are still reachable where they must be: `pub(crate)` would widen
        // them past the folder, so the module itself stays private and `mod.rs` decides by
        // re-exporting what a group needs to name.
        let _ = writeln!(declarations, "mod {name};");
    }
    declarations
}

/// Scans `src/commands`, reports it to the macro, and tells Cargo to rerun when it changes.
///
/// This is the one line an application's `build.rs` needs:
///
/// ```ignore
/// fn main() {
///     clifacet::build::commands();
/// }
/// ```
///
/// It panics on an unreadable folder, which is what a build script should do: a build
/// script's job is to fail the build, and the alternative — reporting an empty folder — is
/// a command that silently disappears. The message names the path.
///
/// # Panics
///
/// Panics when `src/commands` or one of its group folders cannot be read. `CARGO_MANIFEST_DIR`
/// gives the crate root, so a build script run from any working directory scans the right
/// folder.
pub fn commands() {
    commands_in("src/commands");
}

/// Scans `dir` instead of `src/commands`, for a folder that is not at the crate root.
///
/// The path is relative to the crate root for the same reason [`commands`] uses it, and it
/// is the folder the report names, so a macro reads back the same one.
///
/// Every group folder under `dir` is scanned as well, recursively: a group is a folder with
/// its own `commands!`, and that call has to find its own report. A helper folder is not
/// scanned, because nothing registers there — a group never lives under a `_`.
///
/// # Panics
///
/// Panics when `dir` or one of its group folders cannot be read, naming it.
pub fn commands_in(dir: impl AsRef<Path>) {
    let mut reports = Reports::default();
    scan(dir.as_ref(), dir.as_ref(), &mut reports);
    // The report travels to the macro in the environment, because that is the only channel
    // a build script has to a macro in the same crate.
    println!("cargo:rustc-env={VARIABLE}={}", reports.encode());
}

/// Scans one folder into `reports`, and then each of its groups, depth first.
///
/// `dir` is how the folder was named to [`commands_in`], which is the form the report keeps;
/// `folder` is where it actually is on disk. They differ only at the root, where the caller's
/// path is relative to the crate root and the filesystem needs the crate root too.
fn scan(dir: &Path, folder: &Path, reports: &mut Reports) {
    let root = Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default()).join(folder);
    let mut report = Report::of(&root).unwrap_or_else(|error| {
        panic!(
            "clifacet::build: cannot read the command folder {}: {error}",
            root.display()
        )
    });
    // Named the way the macro will look for it: relative to the crate root, because that is
    // the form `Span::local_file()` gives a file the compiler was handed relatively.
    report.folder = dir.to_path_buf();
    // The folder itself, and each entry inside it: Cargo watches an mtime, so a folder's own
    // mtime changes when a file is added to or removed from it, and an entry's does when it is
    // edited. Watching only `mod.rs` would miss a new command file, which is exactly the edit
    // this feature exists to make cheap.
    println!("cargo:rerun-if-changed={}", root.display());
    for entry in &report.entries {
        println!("cargo:rerun-if-changed={}", root.join(entry).display());
    }
    let groups = report.groups.clone();
    reports.folders.push(report);
    for group in groups {
        scan(&dir.join(&group), &folder.join(&group), reports);
    }
}
