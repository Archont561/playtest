//! A value the framework is allowed to carry but not to show.
//!
//! A credential in a [`Secret`] is redacted wherever the framework speaks — a
//! diagnostic an application writes, a preview a screen draws, a record a test keeps —
//! because those places print values through [`Debug`] or [`Display`], and both of
//! [`Secret`]'s implementations are the same marker.
//!
//! ```rust
//! use clifacet::secret::{REDACTED, Secret};
//!
//! let token = Secret::new(String::from("sk-live-1234"));
//!
//! assert_eq!(format!("{token}"), REDACTED);
//! assert_eq!(format!("{token:?}"), "Secret([redacted])");
//! assert_eq!(token.expose(), "sk-live-1234");
//! ```
//!
//! The type is a promise about the framework's own output, not a guarantee about the
//! program. [`Secret::expose`] is deliberately the only way to read the value, so
//! every place that reads it is a place someone wrote, but nothing stops an
//! application from writing it to a log. See `docs/command-core.md` for what this
//! cannot cover.

use std::fmt;
use std::str::FromStr;

/// What replaces a secret wherever the framework shows one.
///
/// A fixed marker rather than a fixed-width mask: masking the characters would still
/// report the length, which is a property of the secret.
pub const REDACTED: &str = "[redacted]";

/// A value that prints as [`REDACTED`].
///
/// It parses with [`FromStr`] so `clap` accepts it as a value, and it compares by
/// value so an application can still test what it received. With the `serde` feature
/// it serialises as [`REDACTED`] too, so a secret inside a serialised value — a JSON
/// report, a recorded run — cannot carry the credential out. It never deserialises:
/// writing a credential into a config file and reading it back is a decision the
/// application has to make explicitly, through [`Secret::expose`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Secret<T> {
    value: T,
}

impl<T> Secret<T> {
    /// Wraps a value that must not be shown.
    #[must_use]
    pub fn new(value: T) -> Self {
        Self { value }
    }

    /// Reads the value.
    ///
    /// Every call is a place that could write the credential somewhere; that is why
    /// it has a name instead of `Deref`.
    #[must_use]
    pub fn expose(&self) -> &T {
        &self.value
    }

    /// Takes the value out, for handing it to something that needs to own it.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for Secret<T> {
    /// Redacted even when `T` would print itself: the whole point of the type.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret([redacted])")
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Secret<T> {
    /// Redacted whatever `T` is, and without asking `T` to serialise at all.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(REDACTED)
    }
}

impl<T: fmt::Display> fmt::Display for Secret<T> {
    /// Redacted even when `T` would print itself.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(REDACTED)
    }
}

impl<T: FromStr> FromStr for Secret<T> {
    type Err = T::Err;

    /// Parses the inner value, so `clap` can accept a secret without a value parser of
    /// its own.
    ///
    /// # Errors
    ///
    /// Returns whatever the inner type rejects. `clap` reports it as a usage error,
    /// which never contains the value it failed to parse for a type whose error does
    /// not quote it.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self::new)
    }
}
