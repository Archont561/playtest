//! What a full-screen form holds, and what a keystroke does to it.
//!
//! The state machine is deliberately free of `ratatui`: a [`Key`] is a key, not a
//! crossterm event, and editing a value never needs a screen. That is what lets the
//! form be driven headlessly, and it keeps the drawing in [`crate::tui::widgets`]
//! where a look change cannot alter behaviour.

use clap::{Arg, ArgAction, Command};

use crate::facet::Facet;
use crate::failure::Failure;

/// One keystroke the form understands.
///
/// A frontend maps its own events onto these, so the state machine does not depend
/// on a terminal library — and a test can press keys without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// A printable character.
    Char(char),
    /// Delete the last character of the focused value.
    Backspace,
    /// Focus the next field, wrapping at the end.
    Tab,
    /// Focus the previous field, wrapping at the start.
    BackTab,
    /// Submit the form.
    Enter,
    /// Leave the form without submitting.
    Escape,
}

/// What the form decided about a keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Stay in the form; the keystroke only changed it.
    Editing,
    /// The user asked to submit. Validation decides what happens next.
    Submit,
    /// The user left the form.
    Cancel,
}

/// What kind of answer a field takes, which decides what a key does to it.
///
/// The value itself is always the text the field would submit, whatever the kind, so
/// drawing, submission and a custom widget all read the same string.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// Free text, typed character by character.
    Text,
    /// One of these names, or none (an empty value).
    Choice(Vec<String>),
    /// A flag: the value is `"true"` or `"false"`, and the bare flag sets `sets`.
    Toggle { sets: bool },
}

/// One question in the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldState {
    name: String,
    label: String,
    value: String,
    kind: Kind,
    required: bool,
    /// Whether the value must not be drawn. Presentation only: the submission still
    /// carries it, because masking is about what a screen shows, not what a command
    /// receives.
    secret: bool,
    /// Whether `value` is still the one the application chose rather than something
    /// the user typed. Typing replaces a default; backspace edits it like any other
    /// text, because deleting a character from a default is an edit, not an answer.
    untouched: bool,
    /// The flag the value is submitted under, when it is not `--{name}`: a generated
    /// field takes it from clap, because an id (`dry_run`) is not a flag (`--dry-run`).
    flag: Option<String>,
}

impl FieldState {
    /// Builds a field directly, for a layout the command metadata cannot describe.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let value = value.into();
        Self {
            name: name.into(),
            label: label.into(),
            untouched: !value.is_empty(),
            value,
            kind: Kind::Text,
            required: false,
            secret: false,
            flag: None,
        }
    }

    /// Builds a toggle: a flag that is either given or not.
    ///
    /// Space flips it, and it submits the bare `--{name}` when it is on — the shape of a
    /// `clap` `SetTrue` flag. A generated form also builds `SetFalse` flags, which submit
    /// theirs when switched *off*.
    #[must_use]
    pub fn toggle(name: impl Into<String>, label: impl Into<String>, on: bool) -> Self {
        let mut field = Self::new(name, label, on.to_string());
        field.kind = Kind::Toggle { sets: true };
        field
    }

    /// Makes this field a choice between `options`, keeping its value as the one chosen.
    ///
    /// Space moves to the next option and a letter jumps to the next option starting
    /// with it; an optional choice also passes through *none*, an empty value, which
    /// submits nothing.
    #[must_use]
    pub fn with_choices(mut self, options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.kind = Kind::Choice(options.into_iter().map(Into::into).collect());
        self
    }

    /// The options of a choice, in order; empty for any other kind of field.
    #[must_use]
    pub fn choices(&self) -> &[String] {
        match &self.kind {
            Kind::Choice(options) => options,
            Kind::Text | Kind::Toggle { .. } => &[],
        }
    }

    /// Whether this field is a toggle.
    #[must_use]
    pub const fn is_toggle(&self) -> bool {
        matches!(self.kind, Kind::Toggle { .. })
    }

    /// Whether this field is a toggle that is on.
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.is_toggle() && self.value == "true"
    }

    /// The arguments this field submits: none, the bare flag, or the flag and a value.
    fn arguments(&self) -> Vec<String> {
        match self.kind {
            // Only a toggle that differs from what absence means is given: the bare flag
            // is the only spelling clap accepts, and an untouched one says nothing.
            Kind::Toggle { sets } => {
                if self.value == sets.to_string() {
                    vec![self.flag()]
                } else {
                    Vec::new()
                }
            }
            // An empty optional value is omitted rather than passed as an empty string,
            // so a field left untouched behaves exactly as if the flag had not been typed.
            Kind::Text | Kind::Choice(_) if self.is_empty() && !self.required => Vec::new(),
            Kind::Text | Kind::Choice(_) => vec![self.flag(), self.value.clone()],
        }
    }

    /// The flag this field's value is submitted under.
    fn flag(&self) -> String {
        self.flag
            .clone()
            .unwrap_or_else(|| format!("--{}", self.name))
    }

    /// Marks this field as required, which the form uses when it builds arguments.
    #[must_use]
    pub fn mark_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Marks this field as secret, so a preview draws [`crate::secret::REDACTED`]
    /// instead of the value.
    #[must_use]
    pub const fn mark_secret(mut self, secret: bool) -> Self {
        self.secret = secret;
        self
    }

    /// Whether the value must not be drawn.
    #[must_use]
    pub const fn is_secret(&self) -> bool {
        self.secret
    }

    /// The `clap` argument id this field answers for.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The question shown to the user.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The value being edited.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Whether the command requires this argument.
    #[must_use]
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Whether the value is still empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Adds a character, replacing a default the user has not touched.
    ///
    /// A prefilled value is an offer, not an answer: appending to it would turn
    /// "change 1 to 3" into "delete the 1 first", which is the kind of small tax that
    /// makes a form worse than the flags it replaces.
    ///
    /// A choice and a toggle take no text: Space moves on or flips, and a letter on a
    /// choice jumps to the next option starting with it.
    fn push(&mut self, character: char) {
        match &self.kind {
            Kind::Text => {
                if self.untouched {
                    self.value.clear();
                    self.untouched = false;
                }
                self.value.push(character);
            }
            Kind::Choice(options) if character == ' ' => self.value = self.next(options),
            Kind::Choice(options) => {
                if let Some(option) = self.jump(options, character) {
                    self.value = option;
                }
            }
            Kind::Toggle { .. } if character == ' ' => {
                self.value = (!self.is_on()).to_string();
            }
            Kind::Toggle { .. } => (),
        }
    }

    /// Erases: the last character of text, or the choice of an optional choice.
    fn pop(&mut self) {
        match self.kind {
            Kind::Text => {
                self.untouched = false;
                self.value.pop();
            }
            Kind::Choice(_) if !self.required => self.value.clear(),
            Kind::Choice(_) | Kind::Toggle { .. } => (),
        }
    }

    /// The option after the chosen one: from none to the first, and past the last back
    /// to none — or round to the first, when the choice is required.
    fn next(&self, options: &[String]) -> String {
        let chosen = options.iter().position(|option| *option == self.value);
        let next = match chosen {
            None => 0,
            Some(index) if index + 1 < options.len() => index + 1,
            Some(_) if self.required => 0,
            Some(_) => return String::new(),
        };
        options.get(next).cloned().unwrap_or_default()
    }

    /// The next option after the chosen one that starts with `letter`, in any case.
    fn jump(&self, options: &[String], letter: char) -> Option<String> {
        let start = options
            .iter()
            .position(|option| *option == self.value)
            .map_or(0, |index| index + 1);
        let starts_with = |option: &&String| {
            option
                .chars()
                .next()
                .is_some_and(|first| first.to_lowercase().eq(letter.to_lowercase()))
        };
        options
            .iter()
            .cycle()
            .skip(start)
            .take(options.len())
            .find(starts_with)
            .cloned()
    }
}

/// The form: its fields, where the cursor is, and the last refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormState {
    fields: Vec<FieldState>,
    focus: usize,
    message: Option<String>,
}

impl FormState {
    /// Builds a form from fields, for an application that does its own layout.
    ///
    /// # Panics
    ///
    /// Panics when `fields` is empty: a form with nothing to ask cannot be submitted
    /// and would look like a bug in the terminal.
    #[must_use]
    pub fn new(fields: Vec<FieldState>) -> Self {
        assert!(!fields.is_empty(), "a form needs at least one field");
        Self {
            fields,
            focus: 0,
            message: None,
        }
    }

    /// Generates a form from a clap command, one field per named argument.
    ///
    /// The list of arguments is explicit on purpose: which values a screen asks for
    /// is a design decision, and guessing it from the grammar would put the
    /// interaction flags of a command in front of a user who already chose a mode.
    /// A derived command declares it with `form` and passes `Cli::form_fields()`.
    /// The label comes from the argument's help, the initial value from its default,
    /// and each answer is submitted under the argument's own flag — `--dry-run` for
    /// the id `dry_run`, `-l` for a short-only one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::execution`] when a name is not an argument of `command`:
    /// that is a mistake in the application, not something a user can fix.
    pub fn generated(command: &Command, names: &[&str]) -> Result<Self, Failure> {
        let mut fields = Vec::with_capacity(names.len());
        for name in names {
            let argument = command
                .get_arguments()
                .find(|argument| argument.get_id() == *name)
                .ok_or_else(|| {
                    Failure::execution(format!(
                        "the command {command_name} has no argument named {name:?}",
                        command_name = command.get_name()
                    ))
                })?;

            let label = argument
                .get_help()
                .map_or_else(|| (*name).to_owned(), ToString::to_string);
            let kind = kind_of(argument)?;
            let value = argument.get_default_values().first().map_or_else(
                || match kind {
                    // clap fills a flag's default in when it builds the command, which a
                    // command fresh from the derive has not done yet.
                    Kind::Toggle { sets } => (!sets).to_string(),
                    Kind::Text | Kind::Choice(_) => String::new(),
                },
                |value| value.to_string_lossy().into_owned(),
            );

            let mut field =
                FieldState::new(*name, label, value).mark_required(argument.is_required_set());
            field.kind = kind;
            // The spelling clap parses, so the submission is the invocation a user would
            // type. A positional keeps `--{name}`, as before: it has no flag to borrow.
            field.flag = argument
                .get_long()
                .map(|long| format!("--{long}"))
                .or_else(|| argument.get_short().map(|short| format!("-{short}")));
            fields.push(field);
        }

        if fields.is_empty() {
            return Err(Failure::execution(
                "a generated form needs at least one argument to ask for",
            ));
        }
        Ok(Self::new(fields))
    }

    /// Generates the form a derived command declares: its `form` fields, with its
    /// `secret` fields masked.
    ///
    /// This is [`FormState::generated`] over [`Facet::form_fields`], plus
    /// [`FormState::mark_secret`] for each of [`Facet::secret_fields`] — the one
    /// constructor that cannot draw a declared credential in plain text because a
    /// second call was forgotten.
    ///
    /// # Errors
    ///
    /// As [`FormState::generated`].
    pub fn of<C: Facet>() -> Result<Self, Failure> {
        let mut form = Self::generated(&<C as Facet>::command(), C::form_fields())?;
        for name in C::secret_fields() {
            form.mark_secret(name);
        }
        Ok(form)
    }

    /// The fields, in the order they are shown.
    #[must_use]
    pub fn fields(&self) -> &[FieldState] {
        &self.fields
    }

    /// Marks the named field as secret, if the form has it.
    ///
    /// A name the form does not ask for is ignored: the field list stays the single
    /// source of truth for what exists.
    pub fn mark_secret(&mut self, name: &str) -> &mut Self {
        if let Some(field) = self.fields.iter_mut().find(|field| field.name == name) {
            field.secret = true;
        }
        self
    }

    /// The field answering for this argument, if the form has one.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&FieldState> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Which field has the focus.
    #[must_use]
    pub const fn focus(&self) -> usize {
        self.focus
    }

    /// The last refusal, shown under the fields.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Shows a refusal on the form.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    /// Applies one keystroke.
    pub fn handle(&mut self, key: Key) -> Outcome {
        match key {
            Key::Char(character) => {
                // Every editable key has its own variant, so a control character here
                // is a terminal artefact rather than an answer — typing it would only
                // put something unrenderable in a value.
                if !character.is_control()
                    && let Some(field) = self.fields.get_mut(self.focus)
                {
                    field.push(character);
                }
                // A refusal belongs to the answer that caused it, so editing clears it.
                self.message = None;
                Outcome::Editing
            }
            Key::Backspace => {
                if let Some(field) = self.fields.get_mut(self.focus) {
                    field.pop();
                }
                self.message = None;
                Outcome::Editing
            }
            Key::Tab => {
                self.focus = (self.focus + 1) % self.fields.len();
                Outcome::Editing
            }
            Key::BackTab => {
                self.focus = (self.focus + self.fields.len() - 1) % self.fields.len();
                Outcome::Editing
            }
            Key::Enter => Outcome::Submit,
            Key::Escape => Outcome::Cancel,
        }
    }

    /// The arguments these answers describe, ready for the command's own parser.
    ///
    /// An empty optional value is omitted rather than passed as an empty string, so
    /// a field left untouched behaves exactly as if the flag had not been typed; a
    /// toggle is given as its bare flag, and only when it differs from its default.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        self.fields.iter().flat_map(FieldState::arguments).collect()
    }
}

/// What kind of field an argument becomes, read from its grammar.
///
/// A flag is a toggle and an argument with possible values is a choice, so the form
/// offers what the parser accepts instead of a text box that rejects the rest. A list —
/// a repeatable or counted flag, or more than one value — is refused: a field submits
/// one value, and cutting a list to its first item would be a silent lie.
fn kind_of(argument: &Arg) -> Result<Kind, Failure> {
    let many = argument
        .get_num_args()
        .is_some_and(|range| range.max_values() > 1);
    match argument.get_action() {
        ArgAction::SetTrue => Ok(Kind::Toggle { sets: true }),
        ArgAction::SetFalse => Ok(Kind::Toggle { sets: false }),
        ArgAction::Set if !many => {
            let options: Vec<String> = argument
                .get_possible_values()
                .iter()
                .filter(|value| !value.is_hide_set())
                .map(|value| value.get_name().to_owned())
                .collect();
            Ok(if options.is_empty() {
                Kind::Text
            } else {
                Kind::Choice(options)
            })
        }
        ArgAction::Set | ArgAction::Append | ArgAction::Count => Err(Failure::execution(format!(
            "{} takes more than one value, and a form field submits one: collect a \
                 list with `Session::collect_values` instead",
            spelling(argument)
        ))),
        _ => Err(Failure::execution(format!(
            "{} is an action, not a value, so a form cannot ask for it",
            spelling(argument)
        ))),
    }
}

/// How a user would type the argument: its long flag, its short one, or its id.
fn spelling(argument: &Arg) -> String {
    argument.get_long().map_or_else(
        || {
            argument.get_short().map_or_else(
                || argument.get_id().to_string(),
                |short| format!("-{short}"),
            )
        },
        |long| format!("--{long}"),
    )
}
