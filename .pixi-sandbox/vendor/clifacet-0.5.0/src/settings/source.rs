//! Choosing and reading the settings file.
//!
//! Three things can name a file, and one thing can refuse all of them:
//!
//! 1. `--config PATH`, before or after the subcommand;
//! 2. `<APP>_CONFIG` in the environment;
//! 3. the platform's config directory, `<config>/<app>/settings.toml`.
//!
//! `--no-config` skips the second and third and makes the first an error, because a run
//! that says "use no file" and also names one is contradicting itself.
//!
//! A file that was *named* and is missing is an error: the run asked for it. A file that
//! was *looked for* in the default place and is missing is not, because most machines have
//! no file and every run on them would fail.
//!
//! The environment is passed in as an [`Environment`], never read directly, so a test can
//! set `--config` through the same path a user does without touching the process.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Environment, Error, Settings};

/// Where the settings file came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// A file named explicitly, by `--config` or `<APP>_CONFIG`.
    Named(PathBuf),
    /// The platform's default file, which was found.
    Default(PathBuf),
    /// No file: none was named, the default place was not used, or no file was there.
    None,
}

impl Chosen {
    /// The file to read, when there is one.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Named(path) | Self::Default(path) => Some(path),
            Self::None => None,
        }
    }
}

/// The two flags that choose a settings file, declared on the command before the parse.
///
/// They are declared here rather than left to [`Globals`](crate::Globals) so that
/// [`App::settings`](crate::App::settings) works for a `Cli` that flattened no globals —
/// the settings flags belong to the settings feature, not to the runner's own options. A
/// command that already declares one of them keeps its declaration, because `clap` refuses
/// a duplicate id and the application's own flag should win.
#[must_use]
pub fn declare(command: clap::Command) -> clap::Command {
    let mut command = command;
    if command
        .get_arguments()
        .all(|argument| argument.get_id() != "config")
    {
        command = command.arg(
            clap::Arg::new("config")
                .long("config")
                .global(true)
                .value_name("PATH")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("Read settings from this file instead of the default location"),
        );
    }
    if command
        .get_arguments()
        .all(|argument| argument.get_id() != "no_config")
    {
        command = command.arg(
            clap::Arg::new("no_config")
                .long("no-config")
                .global(true)
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("config")
                .help("Read no settings file at all"),
        );
    }
    command
}

/// The flags this module reads from an argument list, and their values.
///
/// Parsed by hand rather than by `clap`, because the file has to be chosen before the
/// command is parsed: a file value *is* a `clap` default, so the command must carry it
/// before `clap` reads the argument list at all. That means accepting the two spellings a
/// user would type and nothing else — `--config PATH` and `--config=PATH`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asked {
    /// The path named by `--config`, if it was given.
    pub path: Option<PathBuf>,
    /// Whether `--no-config` was given.
    pub skip: bool,
}

/// Reads `bytes` with `loader`, naming `path` in the report when the loader can.
///
/// A loader that knows the path reports it (`Toml` does, through [`NamedLoader`]); one with
/// only `load` is attributed to the file this chose, so a message never says the
/// placeholder when the reader has a real filename.
fn load_named(loader: &dyn super::Loader, path: &Path, bytes: &[u8]) -> Result<Settings, Error> {
    if let Some(named) = loader.as_named() {
        return named.load_at(path, bytes);
    }
    loader.load(bytes).map_err(|error| {
        Error::new(format!(
            "{}: {}",
            path.display(),
            error
                .message()
                .replace(super::Placeholder::PATH, &path.display().to_string())
        ))
    })
}

impl Asked {
    /// Reads `--config` and `--no-config` out of an argument list, wherever they are.
    ///
    /// Scanning the whole list is what lets `--config` follow the subcommand, and it is
    /// safe because it only looks: `clap` still owns the parse, still owns `--help`, and
    /// still rejects an unknown flag. An argument that looks like `--config` inside
    /// another flag's value is not distinguished here — `clap` will read the same list and
    /// say so, which is the only reading that can be wrong for both.
    #[must_use]
    pub fn in_argv<I, T>(argv: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut asked = Self::default();
        let mut arguments = argv.into_iter().map(Into::into).peekable();
        while let Some(argument) = arguments.next() {
            let Some(text) = argument.to_str() else {
                continue;
            };
            if text == "--config" {
                if let Some(value) = arguments.next() {
                    asked.path = Some(PathBuf::from(value));
                }
            } else if let Some(value) = text.strip_prefix("--config=") {
                asked.path = Some(PathBuf::from(value));
            } else if text == "--no-config" {
                asked.skip = true;
            }
        }
        asked
    }

    /// Which file to read, in the order a user expects.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when `--no-config` and a named file are both given, which is a
    /// contradiction rather than a precedence question.
    pub fn choose(
        &self,
        application: &str,
        environment: &dyn Environment,
    ) -> Result<Chosen, Error> {
        if self.skip {
            return match &self.path {
                Some(path) => Err(Error::new(format!(
                    "--no-config and --config {} ask for opposite things",
                    path.display()
                ))),
                None => Ok(Chosen::None),
            };
        }
        if let Some(path) = &self.path {
            return Ok(Chosen::Named(path.clone()));
        }
        let variable = super::place::variable(application);
        if let Some(path) = environment.var(&variable) {
            if path.is_empty() {
                return Ok(Chosen::None);
            }
            return Ok(Chosen::Named(PathBuf::from(path)));
        }
        Ok(match super::place::default_file(application, environment) {
            Some(path) if path.is_file() => Chosen::Default(path),
            _ => Chosen::None,
        })
    }

    /// The settings the chosen file holds, read with `loader`.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the file cannot be read or the loader rejects it. A named
    /// file that is absent is an error; an absent default file is not.
    pub fn read(
        &self,
        application: &str,
        environment: &dyn Environment,
        loader: &dyn super::Loader,
    ) -> Result<Settings, Error> {
        let chosen = self.choose(application, environment)?;
        let Some(path) = chosen.path() else {
            return Ok(Settings::empty());
        };
        match std::fs::read(path) {
            // A loader that knows the path reports it; `Toml` has `load_at` for exactly
            // this. A loader with only `load` is attributed to the file this chose, so the
            // message never says `<toml>` when the reader has a real filename.
            Ok(bytes) => load_named(loader, path, &bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => match chosen {
                Chosen::Named(_) => Err(Error::new(format!(
                    "{}: named as the settings file, but it does not exist",
                    path.display()
                ))),
                _ => Ok(Settings::empty()),
            },
            Err(error) => Err(Error::new(format!("{}: {error}", path.display()))),
        }
    }
}
