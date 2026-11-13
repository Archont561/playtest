//! Settings from a file, under the flags and the environment.
//!
//! A value can come from four places, and the order is fixed: a flag beats the
//! environment, the environment beats a file, and a file beats the built-in default.
//! This module supplies the third of those. It does not read a file itself — a caller
//! hands it [`Entry`] values, whatever format they were read from — because a library
//! that insists on one format is a library that decides an application's configuration
//! for it. The `settings-toml` feature is the loader most applications want.
//!
//! The values are injected into [`clap`] as defaults, which is what makes the precedence
//! work without a second resolver: `clap` already knows that a flag beats an environment
//! variable beats a default, so a file value only has to arrive as the last of the three.
//! Two consequences follow, and both are `clap`'s rather than this module's:
//!
//! - a file value is parsed by the argument's *own* parser, so `--port 8080` and
//!   `port = 8080` cannot disagree about what a port is;
//! - a value is still validated when a flag overrides it, so a file with a bad value
//!   fails even though the run would never have used it. A settings file that is wrong
//!   is worth knowing about on the run that did not depend on it.
//!
//! # Where a value came from
//!
//! [`Settings::apply`] reports a [`Provenance`] whose records name
//! [`Source::File`](crate::provenance::Source::File), so an application can say where a
//! value came from without that file being the one that supplied it for every argument.
//! The record is corrected after the parse: an argument the file supplied stays
//! [`Source::File`](crate::provenance::Source::File), and one a flag or an environment
//! variable supplied keeps that source, because the file's value was not the one used.
//!
//! # Writing a loader
//!
//! A loader turns bytes into [`Entry`] values and attaches a [`Location`] to each, so an
//! error can name the line. [`Entry::section`] is the subcommand path and [`Entry::key`]
//! is a long flag name; both rules are enforced by [`Settings::apply`], which refuses a
//! key that no argument answers to rather than ignoring it.
//!
//! ```
//! use clifacet::settings::{Entry, Error, Loader, Location, Settings};
//!
//! struct Ini;
//!
//! impl Loader for Ini {
//!     fn load(&self, bytes: &[u8]) -> Result<Settings, Error> {
//!         let text = std::str::from_utf8(bytes).map_err(|error| Error::new(error.to_string()))?;
//!         let mut entries = Vec::new();
//!         for (index, line) in text.lines().enumerate() {
//!             let line = line.trim();
//!             if line.is_empty() || line.starts_with('#') {
//!                 continue;
//!             }
//!             let (key, value) = line
//!                 .split_once('=')
//!                 .ok_or_else(|| Error::new("a line must read `key = value`"))?;
//!             entries.push(
//!                 Entry::new(&[], key.trim(), value.trim())
//!                     .at(Location::new("app.ini").at(index + 1)),
//!             );
//!         }
//!         Settings::new(entries)
//!     }
//! }
//!
//! let settings = Ini.load(b"name = ada\n").expect("a valid file");
//! assert_eq!(settings.entries().len(), 1);
//! ```

use std::fmt;
use std::path::{Path, PathBuf};

use crate::provenance::{Provenance, Source};

/// What went wrong reading or applying settings.
///
/// The message is meant to be read: an error names the file and the line when the loader
/// knew them, and the key in every case, because "the settings file is invalid" is a
/// sentence that tells nobody what to change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    /// An error with a message that already says everything worth saying.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// The message, for a caller that renders errors its own way.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// Where an entry was read from: the file, and the line inside it.
///
/// A loader that cannot report a line — a format that has no lines, or a source that is
/// not a file — omits it, and the error names the file alone. Reporting no location at
/// all is better than reporting a guessed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    path: PathBuf,
    line: Option<usize>,
}

impl Location {
    /// A location in `path`, with no line.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            line: None,
        }
    }

    /// The same location, at `line`.
    #[must_use]
    pub fn at(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// The file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The line, when the loader knew it.
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        self.line
    }
}

impl fmt::Display for Location {
    /// Renders as `file:line`, or the file alone when the line is unknown.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.path.display())?;
        if let Some(line) = self.line {
            write!(formatter, ":{line}")?;
        }
        Ok(())
    }
}

/// One setting: where in the command tree it applies, the flag it names, and its value.
///
/// The section is the subcommand path, outermost first, and is empty for the root
/// command's own flags. The key is a long flag name without its dashes — `port`, not
/// `--port` — because a key is what a writer types into a file, and the file's syntax
/// already decides what punctuation it uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    section: Vec<String>,
    key: String,
    value: String,
    location: Option<Location>,
}

impl Entry {
    /// An entry for `key` in `section`, without a location.
    #[must_use]
    pub fn new(section: &[&str], key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            section: section.iter().map(|name| (*name).to_owned()).collect(),
            key: key.into(),
            value: value.into(),
            location: None,
        }
    }

    /// The same entry, at a place in a file.
    #[must_use]
    pub fn at(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }

    /// The subcommand path this entry applies to, outermost first.
    #[must_use]
    pub fn section(&self) -> &[String] {
        &self.section
    }

    /// The long flag name this entry sets, without its dashes.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The value, as the file wrote it. It is parsed later, by the argument's parser.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Where it was read from, when the loader knew.
    #[must_use]
    pub fn location(&self) -> Option<&Location> {
        self.location.as_ref()
    }

    /// How an error names this entry: the location when it has one, then the key.
    fn describe(&self) -> String {
        match &self.location {
            Some(location) => format!("{location}: {}", self.dotted()),
            None => self.dotted(),
        }
    }

    /// The section and key as one path, for a message: `serve.port`.
    fn dotted(&self) -> String {
        self.section
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(self.key.as_str()))
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// A set of entries, checked for the mistakes this module can see without a command.
///
/// One entry per key: a second entry for a key that is already set is refused rather than
/// silently taking the last, because a file that says two things about one setting has a
/// mistake in it, and the mistake is easier to fix where it is than where it lands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    entries: Vec<Entry>,
}

impl Settings {
    /// Collects entries, refusing one key twice.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] naming both places when two entries set the same section and key.
    pub fn new(entries: Vec<Entry>) -> Result<Self, Error> {
        let mut seen: Vec<(&[String], &str)> = Vec::new();
        for entry in &entries {
            let key = (entry.section(), entry.key());
            if let Some((_, earlier)) = seen
                .iter()
                .find(|(section, name)| *section == entry.section() && *name == entry.key())
            {
                let _ = earlier;
                return Err(Error::new(format!(
                    "{}: `{}` is set twice",
                    entry.describe(),
                    entry.dotted()
                )));
            }
            seen.push(key);
        }
        Ok(Self { entries })
    }

    /// No settings at all, which layers nothing.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Every entry, in the order they were read.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Whether this layers nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry for `key` in `section`, if there is one.
    #[must_use]
    pub fn get(&self, section: &[&str], key: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| {
            entry.key == key
                && entry.section.len() == section.len()
                && entry
                    .section
                    .iter()
                    .zip(section)
                    .all(|(entry_name, wanted)| entry_name == wanted)
        })
    }

    /// The command with every entry injected as a default, and the provenance to match.
    ///
    /// `secret` names the arguments whose values must never be shown, so a value read from
    /// a file is recorded as secret exactly when the argument is — the record follows the
    /// declaration, as it does for every other source.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when an entry names a key that no argument in its section answers
    /// to, when an entry's section is missing from the tree, or when a value is rejected
    /// by the argument's own parser. Each message names the place and the key.
    pub fn apply(&self, command: clap::Command, secret: &[&str]) -> Result<Applied, Error> {
        let mut provenance = Provenance::default();
        let mut injected: Vec<Entry> = Vec::new();
        let mut rest: Vec<Entry> = self.entries.clone();
        let command = Self::walk(
            command,
            &[],
            &mut rest,
            &mut injected,
            &mut provenance,
            secret,
        )?;

        if let Some(entry) = rest.first() {
            return Err(match &entry.location {
                Some(location) => Error::new(format!(
                    "{location}: `{}` does not name an argument here",
                    entry.dotted()
                )),
                None => Error::new(format!(
                    "`{}` does not name an argument here",
                    entry.dotted()
                )),
            });
        }

        Ok(Applied {
            command,
            provenance,
            entries: injected,
        })
    }

    /// Injects the entries belonging to one level, then descends into its subcommands.
    fn walk(
        mut command: clap::Command,
        section: &[String],
        rest: &mut Vec<Entry>,
        injected: &mut Vec<Entry>,
        provenance: &mut Provenance,
        secret: &[&str],
    ) -> Result<clap::Command, Error> {
        // The arguments of this level, as (index of a matching entry, the argument's id).
        let mut matched: Vec<(usize, String, clap::Arg)> = Vec::new();
        for (index, entry) in rest.iter().enumerate() {
            if entry.section.as_slice() != section {
                continue;
            }
            let Some(argument) = command
                .get_arguments()
                .find(|argument| argument.get_long() == Some(entry.key.as_str()))
            else {
                continue;
            };
            matched.push((
                index,
                argument.get_id().as_str().to_owned(),
                argument.clone(),
            ));
        }

        // Highest index first, so removing an entry does not shift the ones still to come.
        for (index, id, argument) in matched.into_iter().rev() {
            let entry = rest.remove(index);
            let mut argument = if takes_value(&argument) {
                parse_one(&argument, &entry)?
            } else {
                // A flag that takes no value is set by being present. A file sets it with
                // `true`; `false` sets nothing, which is the same as leaving it out.
                match flag_state(&entry)? {
                    Flag::Absent => {
                        injected.push(entry);
                        continue;
                    }
                    Flag::Set => argument.default_value("true"),
                }
            };
            // A value supplied here is a default as far as `clap` is concerned, so a
            // required argument would still be reported missing: lift it, because the
            // requirement is satisfied by the file.
            if argument.is_required_set() {
                argument = argument.required(false);
            }
            command = command.mut_arg(&id, |_| argument);
            if secret.contains(&id.as_str()) {
                provenance.record_secret(&id, Source::File);
            } else {
                provenance.record(&id, Source::File);
            }
            injected.push(entry);
        }

        // Descend: the subcommands are rebuilt so a nested entry can reach a leaf.
        let names: Vec<String> = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name().to_owned())
            .collect();
        for name in names {
            let mut path = section.to_vec();
            path.push(name.clone());
            // Descend when the leaf *or anything below it* has entries: `serve` must be
            // entered for `serve.retry.attempts` to reach `retry`, even though no entry
            // names `serve` itself.
            if !rest.iter().any(|entry| entry.section.starts_with(&path)) {
                continue;
            }
            let nested = command
                .find_subcommand(&name)
                .cloned()
                .expect("the name came from this command's own subcommands");
            let nested = Self::walk(nested, &path, rest, injected, provenance, secret)?;
            command = command.mut_subcommand(name, |_| nested);
        }
        Ok(command)
    }
}

/// One setting injected into one argument, and the arguments that used it.
#[derive(Debug, Clone)]
pub struct Applied {
    command: clap::Command,
    provenance: Provenance,
    entries: Vec<Entry>,
}

impl Applied {
    /// The command with the settings in place, ready to parse.
    #[must_use]
    pub fn command(&self) -> &clap::Command {
        &self.command
    }

    /// The command with the settings in place, given up.
    #[must_use]
    pub fn into_command(self) -> clap::Command {
        self.command
    }

    /// The records of the values the file supplied, before the parse corrects them.
    #[must_use]
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// The entries that matched an argument.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Marks in `provenance` the arguments this file actually supplied.
    ///
    /// An entry is injected as a default, so `clap` reports `DefaultValue` for it whether
    /// or not a flag or an environment variable won. This asks `clap` which values it
    /// filled from a default and claims only those, so a file that names a setting a flag
    /// also names is recorded as the flag, which is what happened.
    pub fn claim(self, provenance: &mut Provenance, matches: &clap::ArgMatches) {
        for origin in self.provenance.entries() {
            let id = origin.name();
            let from_default = matches.try_contains_id(id).unwrap_or(false)
                && matches.value_source(id) == Some(clap::parser::ValueSource::DefaultValue);
            if from_default {
                if origin.is_secret() {
                    provenance.record_secret(id, Source::File);
                } else {
                    provenance.record(id, Source::File);
                }
            }
        }
    }
}

/// Whether the argument takes a value, so a file's value means the same thing as a flag.
///
/// A flag that takes no value is not `--quiet=true`: injecting a default through a scratch
/// parse would ask a value parser that does not exist.
fn takes_value(argument: &clap::Arg) -> bool {
    argument.get_action().takes_values()
}

/// What a file says about a flag that takes no value.
enum Flag {
    /// `false`, or nothing to say: a flag is off by default and stays off.
    Absent,
    /// `true`: the flag is on.
    Set,
}

/// Reads a flag's value, refusing anything a flag cannot mean.
///
/// A bool is the only value here — `--quiet = "yes"` is a mistake worth reporting rather
/// than a spelling to accommodate, because the file's author meant one of two things and
/// guessing which of them is worse than asking.
fn flag_state(entry: &Entry) -> Result<Flag, Error> {
    match entry.value() {
        "true" => Ok(Flag::Set),
        "false" => Ok(Flag::Absent),
        other => Err(Error::new(format!(
            "{}: `{}` is a flag, so it is true or false, not `{other}`",
            entry.describe(),
            entry.key()
        ))),
    }
}

/// Runs the argument's own parser over one value, by asking `clap` to parse it.
///
/// `ValueParser::parse_ref` is not public, so the parser is exercised the way a user
/// would: a scratch command holding just this argument is given the value through its
/// own long flag. That is why the failure is `clap`'s own message, word for word.
fn parse_one(argument: &clap::Arg, entry: &Entry) -> Result<clap::Arg, Error> {
    let long = argument
        .get_long()
        .expect("an entry only matches an argument with a long name")
        .to_owned();
    let mut scratch = clap::Command::new("settings")
        .no_binary_name(true)
        .arg(argument.clone().required(false));
    scratch = scratch.disable_help_flag(true).disable_version_flag(true);
    let argv = [format!("--{long}"), entry.value.clone()];
    scratch.try_get_matches_from(argv).map_err(|error| {
        let rendered = error.render().to_string();
        let message = rendered.lines().next().unwrap_or("invalid value").trim();
        Error::new(format!("{}: {message}", entry.describe()))
    })?;
    // `default_values` takes `'static` values unless clap's `string` feature owns them;
    // the feature is enabled precisely so a file's value is owned rather than leaked.
    Ok(argument.clone().default_values([entry.value.clone()]))
}

/// Reads a settings file's bytes into [`Entry`] values.
///
/// A loader is where a format lives. The library supplies one (`settings-toml`, enabled
/// by the `settings-toml` feature) and an application supplies the rest, which is what
/// keeps a library from choosing an application's configuration language.
pub trait Loader {
    /// Reads `bytes`, attaching a [`Location`] to every entry it can.
    ///
    /// The location's path is this loader's own business: [`Placeholder::PATH`] when it does
    /// not know the file, which [`source::Asked::read`](crate::settings::source::Asked::read)
    /// replaces with the real name before reporting.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] describing the first problem it finds, with the line if it can.
    fn load(&self, bytes: &[u8]) -> Result<Settings, Error>;

    /// The same loader, able to name the file it is reading.
    ///
    /// `None` for a loader with one entry point; the caller then attributes the entries to
    /// the file it chose, which is right for any loader that reads one file at a time.
    fn as_named(&self) -> Option<&dyn NamedLoader> {
        None
    }
}

/// A [`Loader`] that can say which file it is reading, so an error names it exactly.
pub trait NamedLoader {
    /// Reads `bytes` from `path`, attributing every entry to it.
    ///
    /// # Errors
    ///
    /// As [`Loader::load`].
    fn load_at(&self, path: &Path, bytes: &[u8]) -> Result<Settings, Error>;
}

/// The name a loader uses for a file it was not told about, and the marker an error is
/// written with so the caller can substitute the real one.
pub struct Placeholder;

impl Placeholder {
    /// What [`Loader::load`] calls the file whose name it does not have.
    pub const PATH: &'static str = "<settings>";
}

/// Where the environment is asked for the variables that choose a file.
///
/// Reading the process environment is a side effect that makes a test depend on the
/// machine it runs on, so it is a value the caller supplies. [`Process`] is the real one;
/// [`Map`] is for tests.
pub trait Environment {
    /// The value of `name`, when it is set.
    fn var(&self, name: &str) -> Option<String>;
}

/// The process's own environment.
#[derive(Debug, Clone, Copy, Default)]
pub struct Process;

impl Environment for Process {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

impl Environment for () {
    fn var(&self, name: &str) -> Option<String> {
        Process.var(name)
    }
}

/// A fixed environment, for a test that must not read the machine's.
#[derive(Debug, Clone, Default)]
pub struct Map {
    pairs: Vec<(String, String)>,
}

impl Map {
    /// An empty environment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same environment, with `name` set to `value`.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.pairs.push((name.into(), value.into()));
        self
    }
}

impl Environment for Map {
    fn var(&self, name: &str) -> Option<String> {
        self.pairs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }
}

/// Choosing and reading the settings file.
pub mod source;

#[cfg(feature = "settings-toml")]
pub mod toml;

/// The config directory, as the platform defines it, and the file name this looks for.
///
/// A missing default file is not an error — most machines have none — so this returning
/// `None` means "no default location on this platform", not "something went wrong".
pub mod place {
    use std::path::PathBuf;

    /// The file name a default settings file has, inside its application's directory.
    pub const FILE: &str = "settings.toml";

    /// The directory an application's configuration lives in, when the platform says.
    #[must_use]
    pub fn config_dir(environment: &dyn super::Environment) -> Option<PathBuf> {
        // Each platform's branch reads only the variables it has. `HOME` is not defined on
        // Windows, and a closure built above the `cfg` blocks would be dead code there —
        // which CI builds warn about and therefore fail on.
        #[cfg(target_os = "windows")]
        {
            let appdata = environment.var("APPDATA")?;
            Some(PathBuf::from(appdata))
        }
        #[cfg(target_os = "macos")]
        {
            let home = environment.var("HOME")?;
            Some(
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support"),
            )
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            if let Some(config) = environment.var("XDG_CONFIG_HOME")
                && !config.is_empty()
            {
                return Some(PathBuf::from(config));
            }
            let home = environment.var("HOME")?;
            Some(PathBuf::from(home).join(".config"))
        }
    }

    /// The default settings file for `application`, if the platform has a place for it.
    #[must_use]
    pub fn default_file(
        application: &str,
        environment: &dyn super::Environment,
    ) -> Option<PathBuf> {
        config_dir(environment).map(|directory| directory.join(application).join(FILE))
    }

    /// The variable that names a settings file for `application`: `<APP>_CONFIG`.
    ///
    /// The application's name is upper-cased and every run of other characters becomes an
    /// underscore, so `backlog-lite` is chosen by `BACKLOG_LITE_CONFIG`.
    #[must_use]
    pub fn variable(application: &str) -> String {
        let mut name = String::with_capacity(application.len() + 7);
        let mut gap = false;
        for character in application.chars() {
            if character.is_ascii_alphanumeric() {
                if gap && !name.is_empty() {
                    name.push('_');
                }
                gap = false;
                name.push(character.to_ascii_uppercase());
            } else {
                gap = true;
            }
        }
        name.push_str("_CONFIG");
        name
    }
}
