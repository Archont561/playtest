//! The companion derive's seam: a clap command that can produce typed input.
//!
//! decision-2 keeps `clap` the grammar authority. This trait therefore does not
//! parse anything itself: it exposes clap's own [`Command`] and bridges an
//! already-parsed struct to the input type a [`Command`](crate::Command) accepts.
//!
//! ```
//! use clifacet::clap::{CommandFactory, Parser};
//! use clifacet::{Facet, Failure};
//!
//! #[derive(clap::Parser, Facet)]
//! #[facet(input = Greeting)]
//! struct Cli {
//!     #[arg(long, alias = "nickname")]
//!     name: Option<String>,
//! }
//!
//! #[derive(Debug, PartialEq)]
//! struct Greeting {
//!     name: Option<String>,
//! }
//!
//! let cli = Cli::try_parse_from(["greet", "--nickname", "ada"]).expect("an alias parses");
//! assert_eq!(cli.validate().expect("valid").name.as_deref(), Some("ada"));
//!
//! // One grammar: the derived command is clap's.
//! assert_eq!(
//!     <Cli as Facet>::command().render_usage().to_string(),
//!     <Cli as CommandFactory>::command().render_usage().to_string(),
//! );
//! ```
//!
//! [`Command`]: clap::Command

use clap::{Command as ClapCommand, CommandFactory};

use crate::failure::Failure;

/// A clap command that bridges to typed, validated input.
///
/// Derive it with `#[derive(clap::Parser, clifacet::Facet)]` and `#[facet(input =
/// Type)]`. The default bridge is a generated `TryFrom<Self> for Input` that copies
/// fields by name, so the two structs have to agree on every field — a missing one is
/// a compile error rather than a silent default — and applies any field rule
/// (`#[facet(fill = …, trim, non_empty, secret)]`) on the way, leaving out a field
/// declared `skip`. A rule across fields is `#[facet(check = path)]`, a
/// `fn(&Input) -> Result<(), Failure>` the generated bridge calls last. When the input
/// is not a field-by-field copy at all, declare the bridge explicitly with
/// `#[facet(bridge = path)]` and a `fn(command) -> Result<Input, Failure>`.
///
/// # Unsupported
///
/// - Enums and unions: the bridge is field by field, and a command is a struct.
/// - Tuple and unit structs, for the same reason.
/// - Types that do not implement [`clap::CommandFactory`] — in practice, a struct
///   that does not derive [`clap::Parser`]. A subcommand [`clap::Args`] value has
///   no command of its own to report.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a clifacet command: it has no typed input to validate into",
    label = "not a command",
    note = "declare it with `#[clifacet::command(input = YourInput)]`, or derive \
            `clap::Parser` and `clifacet::Facet` with `#[facet(input = YourInput)]`"
)]
pub trait Facet: CommandFactory {
    /// The validated input this command produces.
    ///
    /// Every value of this type is runnable, which is what lets
    /// [`Command::execute`](crate::Command::execute) reject nothing.
    type Input;

    /// The clap command, borrowed rather than rebuilt.
    ///
    /// This is [`CommandFactory::command`] and nothing else: aliases, defaults,
    /// groups and value parsers stay clap's, so a derived application cannot
    /// drift into a second grammar.
    #[must_use]
    fn command() -> ClapCommand {
        <Self as CommandFactory>::command()
    }

    /// The arguments a generated form asks for, in the order it shows them.
    ///
    /// The derive lists every field declared with `form`, in declaration order, so the
    /// form is generated from the same declarations as the grammar and the bridge:
    /// `FormState::generated(&<Cli as Facet>::command(), Cli::form_fields())`. Without
    /// a `form` field — or for a hand-written implementation — the list is empty, and
    /// a form generated from it is refused rather than drawn with nothing to ask.
    #[must_use]
    fn form_fields() -> &'static [&'static str] {
        &[]
    }

    /// The arguments declared `secret`, in declaration order.
    ///
    /// A frontend that draws values reads this to mask them, so a credential is hidden
    /// because it was *declared* secret, not because an application remembered to say
    /// so twice — `FormState::of::<Cli>()` is the form that does. Empty by default.
    #[must_use]
    fn secret_fields() -> &'static [&'static str] {
        &[]
    }

    /// Bridges this parsed command to its typed input.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::validation`] when an application rule rejects the
    /// input. Anything clap rejects never reaches this method: a bad flag, a
    /// failed value parser or a missing required argument is a
    /// [`clap::Error`](clap::Error) at parse time.
    fn validate(self) -> Result<Self::Input, Failure>;
}

/// How a diagnostic names an argument: `--long`, `-s`, or `<VALUE>` for a positional.
///
/// The name comes from `clap`'s command rather than from the field, so a renamed flag
/// (`long = "greet-name"`) is reported as the user typed it.
pub(crate) fn describe_argument(command: &ClapCommand, id: &str) -> String {
    let Some(argument) = command
        .get_arguments()
        .find(|argument| argument.get_id() == id)
    else {
        return id.to_owned();
    };
    if let Some(long) = argument.get_long() {
        return format!("--{long}");
    }
    if let Some(short) = argument.get_short() {
        return format!("-{short}");
    }
    argument
        .get_value_names()
        .and_then(<[_]>::first)
        .map_or_else(|| id.to_uppercase(), |name| format!("<{name}>"))
}

/// What the code `#[derive(Facet)]` generates calls. Not an API: the paths and
/// signatures here change with the derive, and nothing outside it should name them.
///
/// Each rule is a trait implemented for exactly the types it makes sense for, so a
/// rule declared on the wrong field is a compile error that says which rule and which
/// type — never a runtime surprise.
#[doc(hidden)]
pub mod __derive {
    use clap::CommandFactory;

    use crate::failure::Failure;
    use crate::secret::Secret;

    /// `fill = …`: the value when neither a flag nor an answer supplied one.
    pub fn fill<T>(value: Option<T>, fallback: impl FnOnce() -> T) -> T {
        value.unwrap_or_else(fallback)
    }

    /// How a diagnostic names the argument `id` of the command `C`.
    #[must_use]
    pub fn flag<C: CommandFactory>(id: &str) -> String {
        super::describe_argument(&C::command(), id)
    }

    /// `trim` and `non_empty`: rules for text, required or optional.
    #[diagnostic::on_unimplemented(
        message = "`trim` and `non_empty` apply to text, and `{Self}` is not text",
        label = "declared on this field",
        note = "the field has to be `String` or `Secret<String>`, or an `Option` of either \
                (after `fill`, if one is declared)"
    )]
    pub trait Text: Sized {
        /// Removes surrounding whitespace.
        #[must_use]
        fn trim(self) -> Self;

        /// Keeps blank text out: a required value fails, an optional one becomes absent.
        ///
        /// # Errors
        ///
        /// Returns [`Failure::validation`] naming the flag when a required value is blank.
        fn non_empty(self, flag: impl FnOnce() -> String) -> Result<Self, Failure>;
    }

    /// The text inside a value, for the rules above.
    trait Blank {
        fn is_blank(&self) -> bool;
        fn trimmed(self) -> Self;
    }

    impl Blank for String {
        // `str::trim` by name: `Text::trim` takes `self` by value and would win.
        fn is_blank(&self) -> bool {
            str::trim(self).is_empty()
        }

        fn trimmed(self) -> Self {
            let trimmed = str::trim(&self);
            if trimmed.len() == self.len() {
                self
            } else {
                trimmed.to_owned()
            }
        }
    }

    impl Blank for Secret<String> {
        fn is_blank(&self) -> bool {
            self.expose().is_blank()
        }

        // Trimmed inside the wrapper: the credential is never a bare `String` here.
        fn trimmed(self) -> Self {
            Self::new(self.into_inner().trimmed())
        }
    }

    macro_rules! text {
        ($($required:ty),*) => {$(
            impl Text for $required {
                fn trim(self) -> Self {
                    self.trimmed()
                }

                fn non_empty(self, flag: impl FnOnce() -> String) -> Result<Self, Failure> {
                    if self.is_blank() {
                        return Err(Failure::validation(format!("{} must not be blank", flag())));
                    }
                    Ok(self)
                }
            }

            // Optional text that is blank was not really supplied: it becomes absent,
            // which leaves the input's own `None` handling in charge.
            impl Text for Option<$required> {
                fn trim(self) -> Self {
                    self.map(Blank::trimmed)
                }

                fn non_empty(self, _flag: impl FnOnce() -> String) -> Result<Self, Failure> {
                    Ok(self.filter(|value| !value.is_blank()))
                }
            }
        )*};
    }

    text!(String, Secret<String>);

    /// `secret`: the field holds a credential, so it has to be a type that cannot print it.
    #[diagnostic::on_unimplemented(
        message = "`secret` needs a redacted type, and `{Self}` would print the credential",
        label = "declared on this field",
        note = "declare the field as `clifacet::secret::Secret<_>`, or an `Option` of one"
    )]
    pub trait Credential {}

    impl<T> Credential for Secret<T> {}
    impl<T> Credential for Option<Secret<T>> {}

    /// Passes a credential through, proving its type at compile time.
    pub fn credential<T: Credential>(value: T) -> T {
        value
    }

    /// A command tree: what `#[clifacet::subcommands(input = …)]` declares.
    ///
    /// A `subcommands` field converts through this, so a field whose type is not a tree
    /// is reported as that, at the field, instead of as a missing `TryFrom`.
    #[diagnostic::on_unimplemented(
        message = "`subcommands` needs a command tree, and `{Self}` is not one",
        label = "declared on this field",
        note = "declare the enum with `#[clifacet::subcommands(input = YourRequest)]`, one \
                leaf struct per variant; the field may be the tree or an `Option` of it"
    )]
    pub trait Tree: crate::prompt::Collect + Sized {
        /// The typed request the tree converts to: one variant per subcommand.
        type Input;

        /// Converts the chosen leaf to its typed input.
        ///
        /// # Errors
        ///
        /// Returns the leaf's own validation [`Failure`].
        fn into_input(self) -> Result<Self::Input, Failure>;
    }

    /// `subcommands` on a required field: the chosen leaf, converted.
    ///
    /// # Errors
    ///
    /// Returns the leaf's own validation [`Failure`].
    pub fn tree<T: Tree>(tree: T) -> Result<T::Input, Failure> {
        tree.into_input()
    }

    /// `subcommands` on an `Option` field: absent stays absent, for the application to
    /// pick a default leaf itself.
    ///
    /// # Errors
    ///
    /// Returns the leaf's own validation [`Failure`].
    pub fn optional_tree<T: Tree>(tree: Option<T>) -> Result<Option<T::Input>, Failure> {
        tree.map(Tree::into_input).transpose()
    }

    /// Collection for a required `subcommands` field: the chosen leaf asks its questions.
    ///
    /// # Errors
    ///
    /// Returns the leaf's collection [`Failure`].
    pub fn collect_tree<T: Tree>(
        tree: T,
        session: &mut crate::prompt::Session,
    ) -> Result<T, Failure> {
        tree.collect(session)
    }

    /// Collection for an `Option` field: nothing is asked when no subcommand was given.
    ///
    /// # Errors
    ///
    /// Returns the leaf's collection [`Failure`].
    pub fn collect_optional_tree<T: Tree>(
        tree: Option<T>,
        session: &mut crate::prompt::Session,
    ) -> Result<Option<T>, Failure> {
        tree.map(|tree| tree.collect(session)).transpose()
    }
}
