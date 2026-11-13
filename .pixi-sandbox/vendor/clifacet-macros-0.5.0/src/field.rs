//! `#[clifacet(..)]` on a field: the argument declared once, as grammar plus interaction.
//!
//! An argument in this crate used to be declared four times — an `#[arg(..)]` for the
//! grammar, a field on the input type, a prompt call, and an entry in the form's
//! hand-written field list — and the three declarative ones are what drift. decision-13
//! makes the first the whole declaration: grammar keys are forwarded into clap's
//! `#[arg(..)]`, and the interaction and normalization keys describe what the generated
//! bridge and prompt collection do with the same field.
//!
//! This module is a forwarder and nothing else. Grammar keys become one `#[arg(..)]`;
//! derive keys (`ask`, `confirm`, `fill`, `trim`, `non_empty`, `secret`, `skip`, `form`,
//! `subcommands`) become one field-level `#[facet(..)]`, which the `Facet` derive reads —
//! so there is still exactly one generator of the bridge and of collection, and this
//! attribute cannot drift from it. Tokens are forwarded, never reinterpreted: the
//! value of `short`, `range` or `value_parser` reaches clap exactly as written, so clap
//! stays the grammar authority (decision-2) and this crate still has no second parser.
//!
//! Two rules keep the forwarding honest:
//!
//! - **Known keys only.** An unknown key is a compile error that lists the supported
//!   ones, because a key that is silently ignored is an argument that silently is not
//!   what it says.
//! - **Once each.** A key declared twice is a compile error rather than a last-one-wins
//!   resolution, which is the failure mode a merged attribute would otherwise hide.
//!
//! Anything the key table does not cover stays available as a raw `#[arg(..)]` on the
//! same field, which passes through untouched — the escape hatch decision-13 keeps open.

use syn::punctuated::Punctuated;
use syn::{Attribute, GenericArgument, Meta, PathArguments, Token, Type};

use crate::rules;

/// The attribute this module reads on a field.
pub(crate) const ATTRIBUTE: &str = "clifacet";

/// Grammar keys that take a value, forwarded as `#[arg(key = value)]`.
///
/// Deliberately a list rather than "everything clap knows": it is the documented
/// surface, it is what the error message offers, and a new clap key is added here when
/// someone needs it. The raw `#[arg(..)]` escape hatch covers the rest immediately.
///
/// The split follows clap's derive exactly: a builder method such as `required` or
/// `hide` is always `key = value` there (`required = true`), so it is here too.
pub(crate) const VALUE_KEYS: &[&str] = &[
    "alias",
    "conflicts_with",
    "conflicts_with_all",
    "default_value",
    "group",
    "help",
    "hide",
    "id",
    "ignore_case",
    "num_args",
    "range",
    "required",
    "requires",
    "short_alias",
    "value_delimiter",
    "value_name",
    "visible_alias",
];

/// Grammar keys that are flags, forwarded as `#[arg(key)]`.
pub(crate) const FLAG_KEYS: &[&str] = &["value_enum"];

/// Grammar keys clap's derive accepts both ways: bare, where clap derives the value
/// (`long` from the field's name, `env` from it in capitals, `default_value_t` from the
/// type's `Default`, `value_parser` and `action` from the type), or with one written
/// out. Forwards whichever was written.
pub(crate) const EITHER_KEYS: &[&str] = &[
    "action",
    "default_value_t",
    "env",
    "long",
    "short",
    "value_parser",
];

/// The key that makes the field a command tree: clap's `#[command(subcommand)]`, and the
/// derive's `subcommands` rule. It has no grammar of its own, so it stands alone.
const SUBCOMMANDS: &str = "subcommands";

/// The rule that marks a credential; it also hides an environment variable's value.
const SECRET: &str = "secret";

/// The one key that is not clap's: `range = 1..=8` becomes clap's ranged value parser
/// for the field's own type, which clap's derive has no attribute for.
const RANGE: &str = "range";

/// One field's `#[clifacet(..)]`, split into what it means.
pub(crate) struct Declared {
    /// The forwarded `#[arg(..)]` entries, in declaration order.
    arguments: Vec<Meta>,
    /// The forwarded field-level `#[facet(..)]` entries, in declaration order.
    rules: Vec<Meta>,
    /// Whether the field is a command tree, which clap reads from `#[command(subcommand)]`.
    subcommand: bool,
}

impl Declared {
    /// The attributes this declaration becomes: clap's `#[arg(..)]` when the field
    /// declared any grammar, and the derive's `#[facet(..)]` when it declared any rule.
    ///
    /// An empty `#[arg()]` is left out rather than emitted: clap reads a field with
    /// none exactly as one with an empty one, and a `#[command(flatten)]` field that
    /// only declares `skip` must not gain an argument attribute it cannot have.
    pub(crate) fn attributes(&self) -> Vec<Attribute> {
        let (arguments, rules) = (&self.arguments, &self.rules);
        let mut attributes: Vec<Attribute> = Vec::new();
        if self.subcommand {
            attributes.push(syn::parse_quote!(#[command(subcommand)]));
        }
        if !arguments.is_empty() {
            attributes.push(syn::parse_quote!(#[arg(#(#arguments),*)]));
        }
        if !rules.is_empty() {
            attributes.push(syn::parse_quote!(#[facet(#(#rules),*)]));
        }
        attributes
    }
}

/// Parse one `#[clifacet(..)]` attribute on a field of type `ty`, or say what is wrong.
pub(crate) fn parse(attribute: &Attribute, ty: &Type) -> syn::Result<Declared> {
    let entries = attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    if entries.is_empty() {
        return Err(syn::Error::new_spanned(
            attribute,
            format!(
                "`#[{ATTRIBUTE}()]` declares nothing: name at least one key, for example \
                 `#[{ATTRIBUTE}(long, value_name = \"NAME\")]`"
            ),
        ));
    }

    let mut declared: Vec<Meta> = Vec::new();
    let mut rules: Vec<Meta> = Vec::new();
    for entry in entries {
        let key = entry.path().get_ident().map(ToString::to_string);
        let Some(key) = key else {
            return Err(syn::Error::new_spanned(
                &entry,
                format!("`#[{ATTRIBUTE}(..)]` keys are plain names: {}", supported()),
            ));
        };
        if declared
            .iter()
            .chain(&rules)
            .any(|existing| existing.path().is_ident(&key))
        {
            return Err(syn::Error::new(
                entry.path().segments[0].ident.span(),
                format!(
                    "`{key}` is declared twice on this field; one key, one meaning — \
                     merge the two declarations"
                ),
            ));
        }
        // The derive checks a rule's shape, so the rule has one definition.
        if is_rule(&key) {
            rules.push(entry);
        } else {
            declared.push(check_key(entry, &key)?);
        }
    }

    // clap's help prints an environment variable's current value beside its name, so a
    // credential that may come from one hides the value. It costs nothing on a field that
    // reads no variable, and it also covers a raw `#[arg(env = ..)]` on the same field.
    if rules.iter().any(|entry| entry.path().is_ident(SECRET)) {
        declared.push(syn::parse_quote!(hide_env_values = true));
    }

    let subcommand = rules.iter().any(|entry| entry.path().is_ident(SUBCOMMANDS));
    if subcommand && let Some(argument) = declared.first() {
        return Err(syn::Error::new_spanned(
            argument,
            "a `subcommands` field is a command tree, not an argument, so it takes no \
             grammar key: declare the arguments on the leaf structs, or a raw \
             `#[command(..)]` on the field for anything clap's subcommand field accepts",
        ));
    }

    Ok(Declared {
        arguments: ranged(declared, ty)?,
        rules,
        subcommand,
    })
}

/// Replaces `range = R` with `value_parser = clap::value_parser!(T).range(R)`.
///
/// `T` is the field's type with `Option` and `Vec` peeled, which is the type clap's
/// derive parses each value as. A `value_parser` beside `range` would be a second
/// parser for one argument, so that is an error rather than a silent choice.
fn ranged(arguments: Vec<Meta>, ty: &Type) -> syn::Result<Vec<Meta>> {
    if !arguments.iter().any(|entry| entry.path().is_ident(RANGE)) {
        return Ok(arguments);
    }
    if let Some(parser) = arguments
        .iter()
        .find(|entry| entry.path().is_ident("value_parser"))
    {
        return Err(syn::Error::new_spanned(
            parser,
            "`range` already builds this argument's value parser, so `value_parser` beside \
             it would be a second one: keep `range`, or write \
             `value_parser = clap::value_parser!(T).range(..)` on its own",
        ));
    }
    let value = parsed_type(ty);
    // `check_key` only lets `range` through with a value, so it is always a name-value
    // pair here; matching on that shape replaces it and leaves every other entry alone.
    Ok(arguments
        .into_iter()
        .map(|entry| match entry {
            Meta::NameValue(range) if range.path.is_ident(RANGE) => {
                let bounds = range.value;
                syn::parse_quote_spanned!(range.path.segments[0].ident.span()=>
                    value_parser = ::clifacet::clap::value_parser!(#value).range(#bounds)
                )
            }
            other => other,
        })
        .collect())
}

/// The `T` of an `Option<T>`, for a declaration that fills one in.
pub(crate) fn optional_inner(path: &syn::TypePath) -> Option<&Type> {
    let last = path.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };
    match (arguments.args.len(), arguments.args.first()) {
        (1, Some(GenericArgument::Type(inner))) => Some(inner),
        _ => None,
    }
}

/// The type clap parses one value of this field as: `Option<T>` and `Vec<T>` peeled.
fn parsed_type(ty: &Type) -> &Type {
    peeled(ty).map_or(ty, parsed_type)
}

/// The `T` of an `Option<T>` or a `Vec<T>`; any other type is parsed as itself.
fn peeled(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if last.ident != "Option" && last.ident != "Vec" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };
    match (arguments.args.len(), arguments.args.first()) {
        (1, Some(GenericArgument::Type(inner))) => Some(inner),
        _ => None,
    }
}

/// Whether the key belongs to the derive's field-level `#[facet(..)]`.
fn is_rule(key: &str) -> bool {
    rules::VALUE_KEYS.contains(&key) || rules::FLAG_KEYS.contains(&key)
}

/// Accept the entry when the key is one this table forwards, and forward it as written.
fn check_key(entry: Meta, key: &str) -> syn::Result<Meta> {
    let bare = matches!(entry, Meta::Path(_));
    let known = VALUE_KEYS.contains(&key) || FLAG_KEYS.contains(&key) || EITHER_KEYS.contains(&key);
    if !known {
        return Err(unknown(&entry, key));
    }
    if bare && VALUE_KEYS.contains(&key) {
        return Err(syn::Error::new_spanned(
            &entry,
            format!("`{key}` takes a value, for example `{key} = \"…\"`"),
        ));
    }
    if !bare && FLAG_KEYS.contains(&key) {
        return Err(syn::Error::new_spanned(
            &entry,
            format!("`{key}` takes no value: write `{key}` on its own, without `= …`"),
        ));
    }

    // The value is forwarded as tokens: a string, a char, a range or a parser expression
    // all mean what clap says they mean, and this crate does not interpret any of them.
    Ok(entry)
}

fn unknown(entry: &Meta, key: &str) -> syn::Error {
    syn::Error::new_spanned(
        entry,
        format!(
            "`{key}` is not a `#[{ATTRIBUTE}(..)]` key, so it would be an argument that \
             means nothing. Supported keys: {}. Any other clap key stays available as a raw \
             `#[arg({key} …)]` on the same field, which this attribute passes through \
             untouched.",
            supported()
        ),
    )
}

fn supported() -> String {
    format!(
        "values — {}; flags — {}; either — {}; derive values — {}; derive flags — {}",
        VALUE_KEYS.join(", "),
        FLAG_KEYS.join(", "),
        EITHER_KEYS.join(", "),
        rules::VALUE_KEYS.join(", "),
        rules::FLAG_KEYS.join(", "),
    )
}
