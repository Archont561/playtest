//! A field's `#[facet(..)]`: how its value is asked for and normalised.
//!
//! The struct-level `#[facet(input = …)]` says *what* the bridge builds; these
//! field-level keys say what happens to one field on the way. They are the
//! interaction and normalization half of decision-13 — `#[clifacet(..)]` forwards them
//! here, so the derive stays the one place that generates a bridge.
//!
//! | Key | Kind | Generated into |
//! | --- | --- | --- |
//! | `ask = "…"` | interaction | `Collect::collect`: asks when no flag supplied the value |
//! | `confirm = "…"` | interaction | `Collect::collect`: confirms a flag that was set |
//! | `fill = expr` | normalization | the bridge: the value when nothing supplied one |
//! | `trim` | normalization | the bridge: surrounding whitespace removed |
//! | `non_empty` | normalization | the bridge: blank text rejected, or made absent if optional |
//! | `secret` | normalization | the bridge: the field must be a redacted type |
//! | `skip` | input | the bridge: the field is not copied; clap still parses it |
//! | `form` | interaction | `Facet::form_fields`: the field is a question on the form |
//! | `subcommands` | input | the bridge and `Collect`: the chosen leaf's input, its questions |
//!
//! The bridge applies its rules in one fixed order — `fill`, `trim`, `non_empty`,
//! `secret` — whatever order they were written in, because "trim, then check blank" is
//! the only order in which `non_empty` means anything.

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprLit, Field, Lit, LitStr, Meta, Token};

/// The attribute these rules live in, shared with the struct-level keys.
pub(crate) const ATTRIBUTE: &str = "facet";

/// Field keys that take a value.
pub(crate) const VALUE_KEYS: &[&str] = &["ask", "confirm", "fill"];

/// Field keys that are flags.
pub(crate) const FLAG_KEYS: &[&str] =
    &["form", "non_empty", "secret", "skip", "subcommands", "trim"];

/// Keys that change the value the generated bridge copies.
const NORMALIZATION_KEYS: &[&str] = &["fill", "non_empty", "secret", "trim"];

/// A question and where it was declared, so a type error can point at the key.
pub(crate) struct Question {
    pub(crate) text: LitStr,
    pub(crate) span: Span,
}

/// Everything one field declared, already checked for shape and combination.
#[derive(Default)]
pub(crate) struct Rules {
    pub(crate) ask: Option<Question>,
    pub(crate) confirm: Option<Question>,
    pub(crate) fill: Option<Expr>,
    pub(crate) trim: Option<Span>,
    pub(crate) non_empty: Option<Span>,
    pub(crate) secret: Option<Span>,
    /// Left out of the input: clap parses it, the generated bridge does not copy it.
    pub(crate) skip: Option<Span>,
    /// A question on the generated form.
    pub(crate) form: Option<Span>,
    /// A command tree: the bridge converts the chosen leaf, `Collect` delegates to it.
    pub(crate) subcommands: Option<Span>,
    /// Every key declared, in order, for the rule that `subcommands` stands alone.
    keys: Vec<(String, Span)>,
    /// The first normalization key, for the errors that name it.
    normalization: Option<(String, Span)>,
}

impl Rules {
    /// Whether the generated bridge has anything to do beyond copying the field.
    pub(crate) fn normalises(&self) -> bool {
        self.normalization.is_some()
    }

    /// Rejects a key only the generated bridge acts on when the struct declared its own.
    pub(crate) fn check_bridge(&self) -> syn::Result<()> {
        let skip = self.skip.map(|span| (String::from("skip"), span));
        let subcommands = self
            .subcommands
            .map(|span| (String::from("subcommands"), span));
        match self.normalization.clone().or(skip).or(subcommands) {
            Some((key, span)) => Err(syn::Error::new(
                span,
                format!(
                    "`{key}` is a rule for the generated bridge, and this struct declares its \
                     own `bridge = …`, which builds the whole input: apply the rule in the \
                     bridge, or drop `bridge` and let the declarations build the input"
                ),
            )),
            None => Ok(()),
        }
    }
}

/// Reads every `#[facet(..)]` on a field into its rules.
pub(crate) fn parse(field: &Field) -> syn::Result<Rules> {
    let mut rules = Rules::default();
    for attribute in field.attrs.iter().filter(|a| a.path().is_ident(ATTRIBUTE)) {
        let entries = attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for entry in entries {
            read(&mut rules, &entry)?;
        }
    }

    if rules.subcommands.is_some()
        && let Some((key, span)) = rules.keys.iter().find(|(key, _)| key != "subcommands")
    {
        return Err(syn::Error::new(
            *span,
            format!(
                "`{key}` beside `subcommands` would apply to the whole tree, and a tree is \
                 not one value: declare the rule on the leaf struct's own field instead"
            ),
        ));
    }

    if let (Some(ask), Some(_)) = (&rules.ask, &rules.confirm) {
        return Err(syn::Error::new(
            ask.span,
            "a field is either asked for (`ask`, a value no flag supplied) or confirmed \
             (`confirm`, a flag that was set), not both",
        ));
    }
    if let (Some(ask), Some(_)) = (&rules.ask, &rules.secret) {
        return Err(syn::Error::new(
            ask.span,
            "`ask` on a `secret` would echo the credential as it is typed, because a declared \
             question is read as plain text: supply it with the flag or its environment \
             variable, or ask for it by hand with `Session::ask_secret`",
        ));
    }
    if rules.secret.is_some()
        && let Some(environment) = shown_environment(&field.attrs)
    {
        return Err(syn::Error::new_spanned(
            environment,
            "a `secret` read from the environment would be shown by `--help`, because clap \
             prints the variable's current value beside its name: add \
             `hide_env_values = true` to this `#[arg(..)]`, or declare the field with \
             `#[clifacet(env, secret)]`, which adds it",
        ));
    }
    if let Some(ask) = &rules.ask
        && !is_named_option(&field.attrs)
    {
        return Err(syn::Error::new(
            ask.span,
            "`ask` needs a named option — `long` or `short` — because the answer is parsed \
             as that flag's value; a positional argument cannot be asked for",
        ));
    }
    if let Some(form) = rules.form
        && !is_named_option(&field.attrs)
    {
        return Err(syn::Error::new(
            form,
            "`form` needs a named option — `long` or `short` — because the form submits each \
             answer as that flag's value; a positional argument cannot be a form field",
        ));
    }
    if let Some(form) = rules.form
        && is_list(&field.ty)
    {
        return Err(syn::Error::new(
            form,
            "`form` on a list would cut it to one value, because a form field submits one \
             answer: collect a list with `Session::collect_values` instead",
        ));
    }
    if let (Some(_), Some((key, span))) = (rules.skip, &rules.normalization) {
        return Err(syn::Error::new(
            *span,
            format!(
                "`{key}` on a `skip` field would do nothing: a skipped field never reaches the \
                 input, so there is no value for the rule to apply to"
            ),
        ));
    }
    Ok(rules)
}

/// Records one key, rejecting unknown keys, repeats and the wrong shape.
fn read(rules: &mut Rules, entry: &Meta) -> syn::Result<()> {
    let Some(ident) = entry.path().get_ident() else {
        return Err(syn::Error::new_spanned(entry, unknown_message("this key")));
    };
    let key = ident.to_string();
    let span = ident.span();
    // Each shape goes to the helper that needs it, so a value key always has its value
    // and a flag key never does — there is no "missing value" case left to handle.
    let repeated = match entry {
        Meta::NameValue(pair) if VALUE_KEYS.contains(&key.as_str()) => {
            record_value(rules, &key, span, &pair.value)?
        }
        Meta::Path(_) if FLAG_KEYS.contains(&key.as_str()) => record_flag(rules, &key, span),
        Meta::Path(_) if VALUE_KEYS.contains(&key.as_str()) => {
            return Err(syn::Error::new(
                span,
                format!("`{key}` takes a value, for example `{key} = \"…\"`"),
            ));
        }
        _ if FLAG_KEYS.contains(&key.as_str()) => {
            return Err(syn::Error::new(
                span,
                format!("`{key}` takes no value: write `{key}` on its own, without `= …`"),
            ));
        }
        _ => return Err(syn::Error::new(span, unknown_message(&format!("`{key}`")))),
    };
    if repeated {
        return Err(syn::Error::new(
            span,
            format!("`{key}` is declared twice on this field; one key, one meaning"),
        ));
    }
    if NORMALIZATION_KEYS.contains(&key.as_str()) && rules.normalization.is_none() {
        rules.normalization = Some((key.clone(), span));
    }
    rules.keys.push((key, span));
    Ok(())
}

/// Records a value key, returning whether it was already declared.
fn record_value(rules: &mut Rules, key: &str, span: Span, value: &Expr) -> syn::Result<bool> {
    Ok(match key {
        "ask" => rules.ask.replace(question(value, span)?).is_some(),
        "confirm" => rules.confirm.replace(question(value, span)?).is_some(),
        _ => rules.fill.replace(value.clone()).is_some(),
    })
}

/// Records a flag key, returning whether it was already declared.
fn record_flag(rules: &mut Rules, key: &str, span: Span) -> bool {
    match key {
        "trim" => rules.trim.replace(span).is_some(),
        "non_empty" => rules.non_empty.replace(span).is_some(),
        "skip" => rules.skip.replace(span).is_some(),
        "form" => rules.form.replace(span).is_some(),
        "subcommands" => rules.subcommands.replace(span).is_some(),
        _ => rules.secret.replace(span).is_some(),
    }
}

/// A question has to be a string literal: it is shown to a person as written.
fn question(value: &Expr, span: Span) -> syn::Result<Question> {
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) => Ok(Question {
            text: text.clone(),
            span,
        }),
        other => Err(syn::Error::new_spanned(
            other,
            "a question is a string literal",
        )),
    }
}

fn unknown_message(key: &str) -> String {
    format!(
        "{key} is not a field key of `#[{ATTRIBUTE}(..)]`; supported: {}, {}",
        VALUE_KEYS
            .iter()
            .map(|key| format!("`{key} = …`"))
            .collect::<Vec<_>>()
            .join(", "),
        FLAG_KEYS
            .iter()
            .map(|key| format!("`{key}`"))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// Whether the field is written as an `Option<…>`, which is how clap's derive tells an
/// optional subcommand from a required one; a type alias hides it from both.
pub(crate) fn is_option(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|last| last.ident == "Option")
}

/// Whether the field holds a list: a `Vec<_>`, or an `Option<Vec<_>>`.
///
/// Read from the type as written, as clap's derive reads it: a type alias hides the
/// list from both, and clap then parses it as one value anyway.
fn is_list(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    let Some(last) = path.path.segments.last() else {
        return false;
    };
    if last.ident == "Vec" {
        return true;
    }
    if last.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return false;
    };
    arguments
        .args
        .iter()
        .any(|argument| matches!(argument, syn::GenericArgument::Type(inner) if is_list(inner)))
}

/// Every entry of the field's `#[arg(..)]` attributes, in order.
fn arguments(attributes: &[Attribute]) -> impl Iterator<Item = Meta> + '_ {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("arg"))
        .filter_map(|attribute| {
            attribute
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
}

/// Whether the field's `#[arg(..)]` names it `long` or `short`, i.e. it is not positional.
fn is_named_option(attributes: &[Attribute]) -> bool {
    arguments(attributes)
        .any(|entry| entry.path().is_ident("long") || entry.path().is_ident("short"))
}

/// The field's `env` entry when clap's help would print the variable's value: an `env`
/// without `hide_env_values` or `hide_env` (written bare or `= true`) beside it.
fn shown_environment(attributes: &[Attribute]) -> Option<Meta> {
    let entries: Vec<Meta> = arguments(attributes).collect();
    let hidden = entries.iter().any(|entry| {
        let hides = entry.path().is_ident("hide_env_values") || entry.path().is_ident("hide_env");
        let set_false = matches!(
            entry,
            Meta::NameValue(syn::MetaNameValue {
                value: Expr::Lit(ExprLit { lit: Lit::Bool(flag), .. }),
                ..
            }) if !flag.value
        );
        hides && !set_false
    });
    if hidden {
        return None;
    }
    entries
        .into_iter()
        .find(|entry| entry.path().is_ident("env"))
}

/// The id clap gives the field: an explicit `#[arg(id = "…")]`, or the field's name.
pub(crate) fn argument_id(field: &Field, name: &syn::Ident) -> String {
    field
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("arg"))
        .filter_map(|attribute| {
            attribute
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
        .find_map(|entry| match entry {
            // Only a string literal names the id here; clap rejects anything else itself.
            Meta::NameValue(syn::MetaNameValue {
                path,
                value:
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(text),
                        ..
                    }),
                ..
            }) if path.is_ident("id") => Some(text.value()),
            _ => None,
        })
        .unwrap_or_else(|| name.to_string())
}
