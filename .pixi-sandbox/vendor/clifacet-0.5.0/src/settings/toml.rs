//! Reading settings from TOML, the format most applications already have.
//!
//! A table is a section and a key is a long flag name, so this file:
//!
//! ```toml
//! [serve]
//! port = 8080
//! host = "0.0.0.0"
//!
//! [serve.retry]
//! attempts = 3
//! ```
//!
//! supplies `--port` and `--host` to the `serve` subcommand, and `--attempts` to
//! `serve retry`. A dotted header is the same as nesting, because TOML says it is.
//!
//! The parser is `toml_edit` rather than `toml` for one reason: it keeps each value's byte
//! span, so a setting that turns out to be wrong is reported at the line it was written on.
//! A loader that can only say "your file is wrong" sends the reader back to the whole file
//! to find the one line; the whole point of layering is that the file is where the mistake
//! lives.

use std::path::Path;

use super::{Entry, Error, Loader, Location, Settings};

/// Reads TOML with a table per section and a key per long flag.
#[derive(Debug, Clone, Copy, Default)]
pub struct Toml;

impl Toml {
    /// Reads `bytes`, attributing each entry to `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] naming the line of a syntax error, of a value whose type is not a
    /// setting, or of the same key set twice.
    pub fn load_from(&self, path: &Path, bytes: &[u8]) -> Result<Settings, Error> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
        let document = text
            .parse::<toml_edit::Document<String>>()
            .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;

        let mut entries = Vec::new();
        let lines = Lines::new(text);
        flatten(
            path,
            &mut Vec::new(),
            document.as_item(),
            &lines,
            &mut entries,
        )?;
        Settings::new(entries)
    }
}

impl Loader for Toml {
    fn load(&self, bytes: &[u8]) -> Result<Settings, Error> {
        self.load_from(Path::new(super::Placeholder::PATH), bytes)
    }

    /// `Toml` knows the path, so a message names the file a reader would open.
    fn as_named(&self) -> Option<&dyn super::NamedLoader> {
        Some(self)
    }
}

impl super::NamedLoader for Toml {
    fn load_at(&self, path: &Path, bytes: &[u8]) -> Result<Settings, Error> {
        self.load_from(path, bytes)
    }
}

/// Turns a byte offset into the line a reader would count, one offset at a time.
///
/// A file is read once and the offsets arrive in any order, so each lookup scans; files of
/// settings are small enough that this is cheaper than carrying an index around.
struct Lines<'a> {
    text: &'a str,
}

impl<'a> Lines<'a> {
    const fn new(text: &'a str) -> Self {
        Self { text }
    }

    /// The 1-based line containing `offset`.
    fn of(&self, offset: usize) -> usize {
        self.text
            .as_bytes()
            .iter()
            .take(offset.min(self.text.len()))
            .filter(|byte| **byte == b'\n')
            .count()
            + 1
    }
}

fn flatten(
    path: &Path,
    section: &mut Vec<String>,
    item: &toml_edit::Item,
    lines: &Lines<'_>,
    entries: &mut Vec<Entry>,
) -> Result<(), Error> {
    let Some(table) = item.as_table_like() else {
        return Err(Error::new(format!(
            "{}: a section must be a table",
            path.display()
        )));
    };
    for (key, item) in table.iter() {
        // `as_table_like` covers a table and an inline table: TOML says they are the same
        // document, so a loader that treats them differently invents a distinction.
        if item.is_table_like() {
            section.push(key.to_owned());
            flatten(path, section, item, lines, entries)?;
            section.pop();
            continue;
        }
        if item.is_none() {
            continue;
        }
        let location = item.span().map_or_else(
            || Location::new(path),
            |span| Location::new(path).at(lines.of(span.start)),
        );
        if let Some(array) = item.as_array_of_tables() {
            let _ = array;
        }
        let names: Vec<&str> = section.iter().map(String::as_str).collect();
        let value = scalar(item, path, section, key)?;
        entries.push(Entry::new(&names, key.to_owned(), value).at(location));
    }
    Ok(())
}

/// A scalar as text, so the argument's own parser sees what a writer would have typed.
///
/// A string is taken as written — no quotes, no escapes, because the string *is* the
/// value. Every other scalar renders as its TOML spelling, so `true`, `8080` and `1.5`
/// reach a parser asked to read a bool, an integer or a float, and a date reaches one that
/// understands a date. An array is refused: a setting is one value, and an argument that
/// takes several is written as one line of text or as several entries.
fn scalar(
    item: &toml_edit::Item,
    path: &Path,
    section: &[String],
    key: &str,
) -> Result<String, Error> {
    let dotted = section.iter().fold(String::new(), |mut dotted, name| {
        dotted.push_str(name);
        dotted.push('.');
        dotted
    }) + key;
    let Some(value) = item.as_value() else {
        return Err(Error::new(format!(
            "{}: `{dotted}` is an array of tables, and a setting is a single value",
            path.display()
        )));
    };
    Ok(match value {
        toml_edit::Value::String(text) => text.value().clone(),
        toml_edit::Value::Integer(number) => number.value().to_string(),
        toml_edit::Value::Float(number) => number.value().to_string(),
        toml_edit::Value::Boolean(flag) => flag.value().to_string(),
        toml_edit::Value::Datetime(when) => when.value().to_string(),
        toml_edit::Value::Array(_) | toml_edit::Value::InlineTable(_) => {
            return Err(Error::new(format!(
                "{}: `{dotted}` holds several values, and a setting is one",
                path.display()
            )));
        }
    })
}

impl Settings {
    /// Reads a TOML file at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the file cannot be read or the TOML is not valid.
    pub fn from_toml(path: &Path) -> Result<Self, Error> {
        let bytes = std::fs::read(path)
            .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
        Toml.load_from(path, &bytes)
    }
}
