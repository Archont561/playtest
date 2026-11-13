//! Where each input value came from, recorded without recording the value.
//!
//! An application that accepts a value from a flag, an environment variable, a
//! default, a prompt or a full-screen form knows which of those it used. [`Provenance`]
//! is where it says so, and it is deliberately unable to say what the value was: an
//! [`Origin`] holds a name, a [`Source`] and whether the value is secret, and nothing
//! else. "The record cannot leak a secret" is therefore a property of the type rather
//! than a rule someone has to remember.
//!
//! ```
//! use clifacet::provenance::{Provenance, Source};
//!
//! let mut provenance = Provenance::default();
//! provenance.record("name", Source::Prompt);
//! provenance.record_secret("token", Source::Environment);
//!
//! assert_eq!(provenance.source_of("token"), Some(Source::Environment));
//! assert_eq!(
//!     provenance.to_string(),
//!     "name <- prompt\ntoken <- environment (secret)",
//! );
//! ```
//!
//! A parsed invocation does not need recording by hand: [`Provenance::from_matches`]
//! reads where `clap` found each value, and [`Provenance::merge`] adds what a session
//! asked for, so the record covers every frontend without the application keeping it.
//!
//! Provenance is not a log. It is the answer to "which frontend supplied this?" — the
//! question a parity test asks after it has already asserted that the answer itself
//! was the same everywhere.

use std::fmt;

use clap::parser::ValueSource;
use clap::{ArgMatches, Command};

/// Where a value came from.
///
/// The variants are the sources the framework can name without guessing. A file read by
/// [`settings`](crate::settings) is [`File`](Self::File); a file an application reads
/// itself is not distinguished from a default, because the framework did not read it and
/// cannot say which of the two it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// A command-line flag or argument.
    Argv,
    /// An environment variable.
    Environment,
    /// A settings file, read before the parse and layered under the environment.
    File,
    /// A default the application chose.
    Default,
    /// A question answered through [`crate::prompt`].
    Prompt,
    /// A field of the [`crate::tui`] form.
    Form,
}

impl Source {
    /// How the source is named in a record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Argv => "argv",
            Self::Environment => "environment",
            Self::File => "file",
            Self::Default => "default",
            Self::Prompt => "prompt",
            Self::Form => "form",
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// One recorded input: what it was called, where it came from, and whether it is
/// secret.
///
/// The value is absent by design — there is no field to put it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    name: String,
    source: Source,
    secret: bool,
}

impl Origin {
    /// The input's name, as the application calls it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where the value came from.
    #[must_use]
    pub const fn source(&self) -> Source {
        self.source
    }

    /// Whether the value is secret, and so was never recorded.
    #[must_use]
    pub const fn is_secret(&self) -> bool {
        self.secret
    }
}

impl fmt::Display for Origin {
    /// Renders as `name <- source`, with `(secret)` when the value is one.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} <- {}", self.name, self.source)?;
        if self.secret {
            formatter.write_str(" (secret)")?;
        }
        Ok(())
    }
}

/// The record of where each input value came from.
///
/// Built from the parse with [`Provenance::from_matches`], which knows whether `clap`
/// found a value on the command line, in the environment or in a default, and
/// completed with what a [`Session`](crate::prompt::Session) asked for through
/// [`Provenance::merge`]. A source the framework cannot see — a configuration file of
/// the application's own — is recorded by hand with [`Provenance::record`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Provenance {
    origins: Vec<Origin>,
}

impl Provenance {
    /// Records where `clap` found every value it parsed, under each argument's id.
    ///
    /// `command` is the one `matches` came from. It is needed because the matches list
    /// argument groups beside arguments — the derive adds one named after the struct —
    /// and cannot tell the two apart; walking the command's own arguments records
    /// values and nothing else.
    ///
    /// A value typed on the command line is [`Source::Argv`], one read from an
    /// argument's `env` is [`Source::Environment`], and one `clap` filled in — a
    /// `default_value`, or `false` for a flag that was not given — is
    /// [`Source::Default`]. An argument with no value records nothing: it has no
    /// source, and recording one would be the lie this function exists to avoid. The
    /// chosen subcommand is walked too, so a leaf's arguments are recorded under their
    /// own ids; a `global` argument, which `clap` propagates to every level, keeps one
    /// entry.
    ///
    /// Nothing is marked secret, because the parse does not know which values are;
    /// [`with_secrets`](Self::with_secrets) says so, and a derived command lists them
    /// in [`Facet::secret_fields`](crate::Facet::secret_fields).
    #[must_use]
    pub fn from_matches(command: &Command, matches: &ArgMatches) -> Self {
        let mut provenance = Self::default();
        provenance.walk(command, matches);
        provenance
    }

    /// Records one level of the parse, then the subcommand it chose.
    fn walk(&mut self, command: &Command, matches: &ArgMatches) {
        for argument in command.get_arguments() {
            let id = argument.get_id().as_str();
            // `try_contains_id` rather than `value_source`, which panics on an id the
            // matches were not built with — a command and matches that disagree.
            if !matches.try_contains_id(id).unwrap_or(false) {
                continue;
            }
            if let Some(source) = matches.value_source(id).and_then(source_of) {
                self.record(id, source);
            }
        }
        if let Some((name, chosen)) = matches.subcommand()
            && let Some(subcommand) = command.find_subcommand(name)
        {
            self.walk(subcommand, chosen);
        }
    }

    /// Marks the named records secret, so they are reported without being shown.
    ///
    /// A name with no record stays unrecorded: marking a value secret does not invent
    /// a source for it.
    #[must_use]
    pub fn with_secrets(mut self, names: &[&str]) -> Self {
        for origin in &mut self.origins {
            if names.contains(&origin.name.as_str()) {
                origin.secret = true;
            }
        }
        self
    }

    /// Adds every record of `later`, replacing an earlier record for the same name.
    ///
    /// This is how a parse and a session compose: the parse records what was supplied,
    /// the session records what it asked for, and a value asked for after the parse
    /// found none is recorded once, as asked.
    pub fn merge(&mut self, later: &Self) -> &mut Self {
        for origin in &later.origins {
            self.insert(&origin.name, origin.source, origin.secret);
        }
        self
    }

    /// Records an input and its source, replacing an earlier record for the same name.
    pub fn record(&mut self, name: &str, source: Source) -> &mut Self {
        self.insert(name, source, false)
    }

    /// Records an input whose value must never be shown.
    pub fn record_secret(&mut self, name: &str, source: Source) -> &mut Self {
        self.insert(name, source, true)
    }

    /// Records an input, keeping one entry per name so a re-ask does not duplicate it.
    fn insert(&mut self, name: &str, source: Source, secret: bool) -> &mut Self {
        let origin = Origin {
            name: name.to_owned(),
            source,
            secret,
        };

        match self.origins.iter_mut().find(|entry| entry.name == name) {
            Some(entry) => *entry = origin,
            None => self.origins.push(origin),
        }
        self
    }

    /// Every recorded input, in the order the names first appeared.
    #[must_use]
    pub fn entries(&self) -> &[Origin] {
        &self.origins
    }

    /// Where the named input came from, if it was recorded.
    #[must_use]
    pub fn source_of(&self, name: &str) -> Option<Source> {
        self.origins
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.source)
    }

    /// Whether the named input was recorded as secret.
    #[must_use]
    pub fn is_secret(&self, name: &str) -> bool {
        self.origins
            .iter()
            .any(|entry| entry.name == name && entry.secret)
    }

    /// How many inputs were recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.origins.len()
    }

    /// Whether nothing was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }
}

impl fmt::Display for Provenance {
    /// One line per input, in the order they were recorded.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, origin) in self.origins.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n")?;
            }
            write!(formatter, "{origin}")?;
        }
        Ok(())
    }
}

/// The record a `clap` value source stands for, if this crate can name it.
///
/// `ValueSource` is non-exhaustive: a source a later `clap` adds is not guessed at.
const fn source_of(source: ValueSource) -> Option<Source> {
    match source {
        ValueSource::CommandLine => Some(Source::Argv),
        ValueSource::EnvVariable => Some(Source::Environment),
        ValueSource::DefaultValue => Some(Source::Default),
        _ => None,
    }
}
