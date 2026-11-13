//! `#[derive(Facet)]`: bridge a clap command to typed, validated input.
//!
//! The derive is deliberately thin, because decision-2 keeps `clap` the grammar
//! authority. It implements `clifacet::Facet`, whose `command()` is clap's own
//! [`Command`][clap::Command], and whose `validate` produces the typed input the
//! command accepts. It never builds a second parser, never re-parses argv and
//! never reaches into clap internals: everything a caller observes about flags,
//! aliases, defaults and value parsers is clap's behavior.
//!
//! There are two ways to produce the input, and only two:
//!
//! - the default, a field-by-field mirror of the command struct, which makes a
//!   renamed or missing field a compile error instead of a silent default. Field
//!   rules (`fill`, `trim`, `non_empty`, `secret`) normalise on the way, `skip` leaves
//!   a field out, and `#[facet(check = path)]` runs a rule across the built input;
//! - `#[facet(bridge = path)]`, for an input the declarations cannot describe, which
//!   receives the parsed command and builds the whole input itself.
//!
//! Collection is generated the same way — one question per `ask`/`confirm`, delegation
//! per `subcommands` field — unless `#[facet(collect = path)]` hands it to a function,
//! for questions the declarations cannot describe.

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::parse::{ParseStream, Parser as _};
use syn::spanned::Spanned as _;
use syn::{Data, DeriveInput, Expr, ExprPath, Fields, Generics, Ident, Path, Token, Type};

use crate::rules::{self, Rules};

// The attribute this derive reads, on the struct and on its fields.
use crate::rules::ATTRIBUTE;

/// The keys the attribute accepts.
const SUPPORTED_KEYS: &str = "`input`, `bridge`, `check`, `collect`";

/// What the `#[facet(...)]` attribute declared.
#[derive(Default)]
struct Declared {
    /// Any type the generated impls can name; `crate::input_type` says which.
    input: Option<Type>,
    /// Kept whole, so a qualified `<T as Trait>::build` keeps its `<T as Trait>`.
    bridge: Option<ExprPath>,
    /// A rule across the built input, run by the generated bridge after the field rules.
    check: Option<ExprPath>,
    /// Collection by hand, in place of the generated questions.
    collect: Option<ExprPath>,
}

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fields = named_fields(input)?;
    let declared = parse_attribute(input)?;
    let input_type = declared.input.ok_or_else(|| {
        syn::Error::new_spanned(
            input,
            "the Facet derive needs the typed input to build: \
             add `#[facet(input = YourInputType)]`",
        )
    })?;
    crate::input_type::check(&input_type, &input.generics, declared.bridge.is_some())?;
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let command = quote!(#name #type_generics);

    // Either generate the bridge from the field declarations, or hand over to the
    // declared one. A declared bridge receives the parsed command by value and owns
    // its own normalization, which is why a field rule beside it is an error rather
    // than a rule that silently does nothing.
    let (validate, bridge_impl) = if let Some(bridge) = declared.bridge {
        if let Some(check) = &declared.check {
            return Err(syn::Error::new_spanned(
                check,
                "`check` runs inside the generated bridge, and this struct declares its own \
                 `bridge = …`, which builds the whole input: call the check from the bridge, \
                 or drop `bridge` and let the declarations build the input",
            ));
        }
        for field in &fields {
            field.rules.check_bridge()?;
        }
        (quote!(#bridge(self)), quote!())
    } else {
        let assignments = fields
            .iter()
            .filter(|field| field.rules.skip.is_none())
            .map(|field| field.assignment(&command));
        // The check sees the input the field rules built, never the raw flags: by the
        // time it runs, every value is filled, trimmed and known not to be blank.
        let check = declared.check.as_ref().map(|check| {
            quote_spanned! {check.span()=>
                let checked: fn(&#input_type) -> ::core::result::Result<(), ::clifacet::Failure> =
                    #check;
                checked(&input)?;
            }
        });
        let bridge_impl = quote! {
            impl #impl_generics ::core::convert::TryFrom<#command> for #input_type #where_clause {
                type Error = ::clifacet::Failure;

                fn try_from(
                    cli: #command,
                ) -> ::core::result::Result<Self, ::clifacet::Failure> {
                    // `Self`, not the type as written: `Greeting<'a> { .. }` and
                    // `<T as Shape>::Input { .. }` are not expressions, `Self { .. }` is.
                    let input = Self { #(#assignments),* };
                    #check
                    ::core::result::Result::Ok(input)
                }
            }
        };
        (
            quote!(<#input_type as ::core::convert::TryFrom<Self>>::try_from(self)),
            bridge_impl,
        )
    };
    let collect = match &declared.collect {
        Some(path) => declared_collect(name, &input.generics, &fields, path)?,
        None => collect_impl(name, &input.generics, &fields),
    };
    let form_fields = form_fields(&fields);
    let secret_fields = secret_fields(&fields);

    Ok(quote! {
        impl #impl_generics ::clifacet::Facet for #command #where_clause {
            type Input = #input_type;

            #form_fields
            #secret_fields

            fn validate(
                self,
            ) -> ::core::result::Result<#input_type, ::clifacet::Failure> {
                #validate
            }
        }

        #bridge_impl

        #collect
    })
}

/// One field of the command struct, with what it declared.
struct Declaration {
    name: Ident,
    /// Whether the field is written as an `Option<…>`, read from the type as clap reads it.
    optional: bool,
    /// The id clap knows the field by, for asking and for naming it in a message.
    id: String,
    rules: Rules,
}

impl Declaration {
    /// The field's entry in the generated struct literal: copied by name, with the
    /// declared rules applied in their one fixed order.
    fn assignment(&self, command: &TokenStream) -> TokenStream {
        let name = &self.name;
        if let Some(span) = self.rules.subcommands {
            // Spanned at the key, the field access too: a field whose type is not a tree
            // is reported there, on one line.
            let field = Ident::new(&name.to_string(), span);
            return if self.optional {
                quote_spanned!(span=> #name: ::clifacet::facet::__derive::optional_tree(cli.#field)?)
            } else {
                quote_spanned!(span=> #name: ::clifacet::facet::__derive::tree(cli.#field)?)
            };
        }
        if !self.rules.normalises() {
            return quote!(#name: cli.#name);
        }
        let id = &self.id;
        let mut steps = Vec::new();
        if let Some(fallback) = &self.rules.fill {
            steps.push(quote_spanned! {fallback.span()=>
                let value = ::clifacet::facet::__derive::fill(
                    value,
                    || ::core::convert::Into::into(#fallback),
                );
            });
        }
        if let Some(span) = self.rules.trim {
            steps.push(quote_spanned! {span=>
                let value = ::clifacet::facet::__derive::Text::trim(value);
            });
        }
        if let Some(span) = self.rules.non_empty {
            steps.push(quote_spanned! {span=>
                let value = ::clifacet::facet::__derive::Text::non_empty(
                    value,
                    || ::clifacet::facet::__derive::flag::<#command>(#id),
                )?;
            });
        }
        if let Some(span) = self.rules.secret {
            steps.push(quote_spanned! {span=>
                let value = ::clifacet::facet::__derive::credential(value);
            });
        }
        quote!(#name: { let value = cli.#name; #(#steps)* value })
    }
}

/// `Collect` for the command: ask what no flag supplied, confirm what was set.
///
/// Generated for every derived command, with nothing to do when no field declares a
/// question, so collection is one call whatever the command declares.
fn collect_impl(name: &Ident, generics: &Generics, fields: &[Declaration]) -> TokenStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let asks_by_flag = fields
        .iter()
        .any(|field| field.rules.ask.is_some() || field.rules.confirm.is_some());
    let questions: Vec<_> = fields
        .iter()
        .filter_map(|field| {
            let (field_name, id) = (&field.name, &field.id);
            if let Some(span) = field.rules.subcommands {
                // The chosen leaf asks its own questions, wherever it sits in the tree.
                let field_name = Ident::new(&field_name.to_string(), span);
                return Some(if field.optional {
                    quote_spanned! {span=>
                        self.#field_name = ::clifacet::facet::__derive::collect_optional_tree(
                            self.#field_name,
                            session,
                        )?;
                    }
                } else {
                    quote_spanned! {span=>
                        self.#field_name =
                            ::clifacet::facet::__derive::collect_tree(self.#field_name, session)?;
                    }
                });
            }
            if let Some(ask) = &field.rules.ask {
                let text = &ask.text;
                // Spanned at the key: an `ask` on a field that is not an `Option` is a
                // type error, and that is where the user has to look.
                return Some(quote_spanned! {ask.span=>
                    session.ask_for(&mut self.#field_name, &command, #id, #text)?;
                });
            }
            field.rules.confirm.as_ref().map(|confirm| {
                let text = &confirm.text;
                quote_spanned! {confirm.span=>
                    session.confirm_flag(self.#field_name, &command, #id, #text)?;
                }
            })
        })
        .collect();

    if questions.is_empty() {
        return quote! {
            impl #impl_generics ::clifacet::prompt::Collect for #name #type_generics #where_clause {
                fn collect(
                    self,
                    _session: &mut ::clifacet::prompt::Session,
                ) -> ::core::result::Result<Self, ::clifacet::Failure> {
                    ::core::result::Result::Ok(self)
                }
            }
        };
    }
    // Only a question asked by flag needs clap's command, to parse the answer with.
    let command = asks_by_flag
        .then(|| quote!(let command = <Self as ::clifacet::clap::CommandFactory>::command();));
    quote! {
        impl #impl_generics ::clifacet::prompt::Collect for #name #type_generics #where_clause {
            fn collect(
                mut self,
                session: &mut ::clifacet::prompt::Session,
            ) -> ::core::result::Result<Self, ::clifacet::Failure> {
                #command
                #(#questions)*
                ::core::result::Result::Ok(self)
            }
        }
    }
}

/// `Collect` for the command by the declared `collect = path`, called as
/// `path(self, session) -> Result<Self, Failure>`.
///
/// It owns every question, so a field's `ask` or `confirm` beside it is an error rather
/// than a question that is silently never asked — the same rule `bridge` keeps for the
/// field rules.
fn declared_collect(
    name: &Ident,
    generics: &Generics,
    fields: &[Declaration],
    path: &ExprPath,
) -> syn::Result<TokenStream> {
    let asked = fields.iter().find_map(|field| {
        let ask = field.rules.ask.as_ref().map(|ask| ("ask", ask.span));
        ask.or_else(|| {
            field
                .rules
                .confirm
                .as_ref()
                .map(|confirm| ("confirm", confirm.span))
        })
    });
    if let Some((key, span)) = asked {
        return Err(syn::Error::new(
            span,
            format!(
                "`{key}` is a question for the generated collection, and this struct declares \
                 its own `collect = …`, which asks every question: ask it there, or drop \
                 `collect` and let the declarations ask"
            ),
        ));
    }
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    // The `fn` pointer states the signature, so a wrong one is reported at the path.
    Ok(quote_spanned! {path.span()=>
        impl #impl_generics ::clifacet::prompt::Collect for #name #type_generics #where_clause {
            fn collect(
                self,
                session: &mut ::clifacet::prompt::Session,
            ) -> ::core::result::Result<Self, ::clifacet::Failure> {
                let collect: fn(
                    Self,
                    &mut ::clifacet::prompt::Session,
                ) -> ::core::result::Result<Self, ::clifacet::Failure> = #path;
                collect(self, session)
            }
        }
    })
}

/// `Facet::form_fields` for the command: every `form` field, in declaration order.
///
/// Nothing is generated without one, so the trait's empty default stays in charge.
fn form_fields(fields: &[Declaration]) -> TokenStream {
    let ids: Vec<_> = fields
        .iter()
        .filter(|field| field.rules.form.is_some())
        .map(|field| &field.id)
        .collect();
    if ids.is_empty() {
        return quote!();
    }
    quote! {
        fn form_fields() -> &'static [&'static str] {
            &[#(#ids),*]
        }
    }
}

/// `Facet::secret_fields` for the command: every `secret` field, in declaration order.
///
/// As with the form, nothing is generated without one.
fn secret_fields(fields: &[Declaration]) -> TokenStream {
    let ids: Vec<_> = fields
        .iter()
        .filter(|field| field.rules.secret.is_some())
        .map(|field| &field.id)
        .collect();
    if ids.is_empty() {
        return quote!();
    }
    quote! {
        fn secret_fields() -> &'static [&'static str] {
            &[#(#ids),*]
        }
    }
}

/// Collects the fields the bridge copies, rejecting shapes it cannot bridge.
fn named_fields(input: &DeriveInput) -> syn::Result<Vec<Declaration>> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "the Facet derive bridges a clap command struct to typed input; \
             enums and unions are not supported",
        ));
    };

    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "the Facet derive bridges a command struct field by field; \
             tuple and unit structs are not supported",
        ));
    };

    named
        .named
        .iter()
        .filter_map(|field| field.ident.clone().map(|name| (field, name)))
        .map(|(field, name)| {
            Ok(Declaration {
                optional: rules::is_option(&field.ty),
                id: rules::argument_id(field, &name),
                rules: rules::parse(field)?,
                name,
            })
        })
        .collect()
}

/// Reads the `#[facet(...)]` attribute, rejecting anything it cannot honor.
fn parse_attribute(input: &DeriveInput) -> syn::Result<Declared> {
    let mut declared = Declared::default();
    for attribute in &input.attrs {
        if attribute.path().is_ident(ATTRIBUTE) {
            attribute
                .parse_args_with(|stream: ParseStream<'_>| parse_entries(&mut declared, stream))?;
        }
    }
    Ok(declared)
}

/// Whether `#[clifacet::command(..)]`'s arguments declare an input, read by the same
/// parser as `#[facet(..)]` so the two attributes accept exactly the same values.
pub(crate) fn declares_input(arguments: TokenStream) -> syn::Result<bool> {
    let mut declared = Declared::default();
    (|stream: ParseStream<'_>| parse_entries(&mut declared, stream)).parse2(arguments)?;
    Ok(declared.input.is_some())
}

/// Reads comma-separated `key = value` entries.
///
/// Not `syn::Meta`: its value is an expression, and a type such as `Greeting<String>` is
/// not one. Each key reads its own kind of value instead — a type for `input`, a path for
/// `bridge`, `check` and `collect`.
fn parse_entries(declared: &mut Declared, stream: ParseStream<'_>) -> syn::Result<()> {
    while !stream.is_empty() {
        parse_entry(declared, stream)?;
        if !stream.is_empty() {
            stream.parse::<Token![,]>()?;
        }
    }
    Ok(())
}

fn parse_entry(declared: &mut Declared, stream: ParseStream<'_>) -> syn::Result<()> {
    let key = stream.call(Path::parse_mod_style)?;
    if !stream.peek(Token![=]) {
        return Err(syn::Error::new_spanned(
            &key,
            format!(
                "`#[{ATTRIBUTE}(...)]` takes `key = value` pairs; supported keys: {SUPPORTED_KEYS}"
            ),
        ));
    }
    let Some(key) = key.get_ident() else {
        return Err(syn::Error::new_spanned(
            &key,
            format!("unsupported `#[{ATTRIBUTE}(...)]` key; supported keys: {SUPPORTED_KEYS}"),
        ));
    };
    let name = key.to_string();
    let declared_before = match name.as_str() {
        "input" => declared.input.is_some(),
        "bridge" => declared.bridge.is_some(),
        "check" => declared.check.is_some(),
        "collect" => declared.collect.is_some(),
        unknown => {
            return Err(syn::Error::new_spanned(
                key,
                format!(
                    "unsupported `#[{ATTRIBUTE}(...)]` key `{unknown}`; \
                     supported keys: {SUPPORTED_KEYS}"
                ),
            ));
        }
    };
    if declared_before {
        return Err(syn::Error::new_spanned(
            key,
            format!("`{key}` is declared more than once"),
        ));
    }
    stream.parse::<Token![=]>()?;
    match name.as_str() {
        "input" => declared.input = Some(parse_type(stream)?),
        "bridge" => declared.bridge = Some(parse_path(key, stream)?),
        "collect" => declared.collect = Some(parse_path(key, stream)?),
        _ => declared.check = Some(parse_path(key, stream)?),
    }
    Ok(())
}

/// The value of `input`: a type, reported at the token that stops it being one.
fn parse_type(stream: ParseStream<'_>) -> syn::Result<Type> {
    stream.parse::<Type>().map_err(|error| {
        syn::Error::new(
            error.span(),
            "`input` takes a type, for example `input = my_module::MyInput`",
        )
    })
}

/// The value of `bridge`, `check` or `collect`: a path to a function, qualification included.
fn parse_path(key: &Ident, stream: ParseStream<'_>) -> syn::Result<ExprPath> {
    match stream.parse::<Expr>()? {
        Expr::Path(path) => Ok(path),
        value => Err(syn::Error::new_spanned(
            value,
            format!("`{key}` takes a path, for example `#[{ATTRIBUTE}({key} = my_module::item)]`"),
        )),
    }
}
