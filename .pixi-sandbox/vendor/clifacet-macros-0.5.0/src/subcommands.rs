//! `#[clifacet::subcommands(...)]`: a command tree declared where its commands live.
//!
//! decision-14 makes nesting declarative without giving up clap's ownership of parsing:
//! the enum this attribute wraps is still clap's `Subcommand`, with one variant per
//! subcommand and one leaf struct per variant. What the attribute adds is the part an
//! application used to write by hand next to it — and get wrong silently:
//!
//! - `TryFrom<Tree> for Request`, which validates the chosen leaf by its own `Facet` and
//!   wraps the result in the `Request` variant of the same name. `Request` is written by
//!   the application, so a missing, extra or mistyped variant is a compile error here;
//! - `Collect`, which lets the chosen leaf ask its own questions;
//! - with `output = Type`, `Command`, which runs the chosen leaf with its own input;
//! - with `planned` as well, `Planned` instead, which hands the chosen leaf's plan up,
//!   so `Command` comes from the library's blanket impl and a dry run sees the leaf;
//! - with `context = Type` instead, `Handler<Type>`, which hands the context to the chosen
//!   leaf's own `Handler` — and every `Command` is one, so a leaf without context still
//!   runs — and previews through the leaf's own `Handler::preview` (decision-19 item 2).
//!
//! The rules it keeps, each a compile error that says what to write instead:
//!
//! - **Leaves are structs.** A variant is `Name(Leaf)`; fields written inline in the
//!   variant, or no leaf at all, would be a command with no `Facet` and no input, which
//!   is exactly what this crate exists to rule out.
//! - **Known keys only, once each.** On the attribute and on a variant, like everywhere
//!   else in this crate.
//! - **One name, one subcommand.** A name or alias used twice is reported at the second
//!   one, rather than left for clap's debug assertion at run time.
//!
//! Depth needs nothing of its own: a leaf whose struct declares a `subcommands` field is
//! an inner node, converted and collected through that field.

use std::collections::HashMap;

use heck::ToKebabCase as _;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::parse::{ParseStream, Parser as _};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned as _;
use syn::{Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Ident, Lit, Meta, Token, Type};

/// The attribute's own name, for messages.
const ATTRIBUTE: &str = "clifacet::subcommands";

/// What a variant's `#[clifacet(..)]` accepts.
const VARIANT_ATTRIBUTE: &str = "clifacet";

/// The keys the attribute accepts, for messages.
pub(crate) const SUPPORTED_KEYS: &str =
    "`input = Type`, `output = Type`, `planned`, `context = Type`";

/// Variant keys that take a string, forwarded as `#[command(key = "…")]`.
const VARIANT_VALUE_KEYS: &[&str] = &["alias", "name", "visible_alias"];

/// Variant keys that are flags, forwarded as `#[command(key = true)]`.
const VARIANT_FLAG_KEYS: &[&str] = &["hide"];

/// Variant keys whose value is a name the subcommand answers to.
const NAMING_KEYS: &[&str] = &["alias", "name", "visible_alias"];

/// What the attribute declared.
struct Declared {
    input: Type,
    output: Option<Type>,
    planned: Option<Span>,
    context: Option<Type>,
}

/// One variant: the subcommand's leaf, and the names it answers to.
struct Leaf {
    variant: Ident,
    leaf: Type,
}

pub(crate) fn expand(attribute: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let mut parsed = syn::parse2::<DeriveInput>(item)?;
    let declared = parse_attribute(attribute)?;
    let Data::Enum(data) = &mut parsed.data else {
        return Err(syn::Error::new_spanned(
            &parsed.ident,
            format!(
                "`#[{ATTRIBUTE}]` applies to an enum, one variant per subcommand; a struct \
                 with arguments is a leaf: declare it with `#[clifacet::command(input = …)]`"
            ),
        ));
    };
    if !parsed.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &parsed.generics,
            "a command tree takes no generics: its variants are the application's own \
             leaf structs, and `Request` names each of them",
        ));
    }
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &parsed.ident,
            "a command tree needs at least one subcommand: add a variant `Name(Name)` for \
             a leaf struct `Name`",
        ));
    }
    reject_duplicate_derive(&parsed.attrs)?;

    let renamed = parsed.attrs.iter().any(renames_all);
    let mut leaves = Vec::new();
    let mut names: HashMap<String, Ident> = HashMap::new();
    for variant in &mut data.variants {
        let leaf = leaf_of(variant)?;
        // Raw names first: forwarding turns `#[clifacet(..)]` into `#[command(..)]` too.
        let raw = raw_names(&variant.attrs);
        let mut answers_to = forward_variant_attributes(&mut variant.attrs)?;
        answers_to.extend(raw);
        let mut declared: Vec<(String, String, Span)> = answers_to
            .into_iter()
            .map(|(key, name, span)| (format!("`{key} = \"{name}\"`"), name, span))
            .collect();
        if !declared
            .iter()
            .any(|(label, _, _)| label.starts_with("`name "))
            && !renamed
        {
            let default = variant.ident.to_string().to_kebab_case();
            let label = format!("`{}`, named `{default}` by default,", variant.ident);
            declared.insert(0, (label, default, variant.ident.span()));
        }
        for (label, name, span) in declared {
            if let Some(first) = names.get(&name) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "{label} answers to `{name}`, and so does `{first}`: one name, one \
                         subcommand — rename one of them, or drop the alias"
                    ),
                ));
            }
            names.insert(name, variant.ident.clone());
        }
        leaves.push(Leaf {
            variant: variant.ident.clone(),
            leaf,
        });
    }

    parsed
        .attrs
        .insert(0, syn::parse_quote!(#[derive(::clap::Subcommand)]));
    let name = &parsed.ident;
    let conversion = conversion(name, &declared.input, &leaves);
    let collect = collect(name, &leaves);
    let run = match (&declared.output, declared.planned, &declared.context) {
        (Some(output), _, Some(context)) => {
            handler(name, &declared.input, output, context, &leaves)
        }
        (Some(output), Some(_), None) => planned(name, &declared.input, output, &leaves),
        (Some(output), None, None) => command(name, &declared.input, output, &leaves),
        (None, _, _) => quote!(),
    };

    Ok(quote! {
        #parsed
        #conversion
        #collect
        #run
    })
}

/// Reads `input = Type, output = Type, planned, context = Type`, rejecting anything else by
/// name.
fn parse_attribute(attribute: TokenStream) -> syn::Result<Declared> {
    let mut input: Option<Type> = None;
    let mut output: Option<Type> = None;
    let mut planned: Option<Span> = None;
    let mut context: Option<(Type, Span)> = None;
    let parser = |stream: ParseStream<'_>| -> syn::Result<()> {
        while !stream.is_empty() {
            let key: Ident = stream.parse()?;
            let name = key.to_string();
            let repeated = match name.as_str() {
                "input" => input.is_some(),
                "output" => output.is_some(),
                "planned" => planned.is_some(),
                "context" => context.is_some(),
                unknown => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "`{unknown}` is not a key of `#[{ATTRIBUTE}(..)]`; supported keys: \
                             {SUPPORTED_KEYS}"
                        ),
                    ));
                }
            };
            if repeated {
                return Err(syn::Error::new(
                    key.span(),
                    format!("`{name}` is declared twice; one key, one meaning"),
                ));
            }
            if name == "planned" {
                if stream.peek(Token![=]) {
                    return Err(syn::Error::new(
                        key.span(),
                        "`planned` takes no value: write `planned` on its own",
                    ));
                }
                planned = Some(key.span());
            } else {
                stream.parse::<Token![=]>().map_err(|_| {
                    syn::Error::new(
                        key.span(),
                        format!("`{name}` takes a type, for example `{name} = MyType`"),
                    )
                })?;
                let ty: Type = stream.parse()?;
                match name.as_str() {
                    "input" => input = Some(ty),
                    "context" => context = Some((ty, key.span())),
                    _ => output = Some(ty),
                }
            }
            if !stream.is_empty() {
                stream.parse::<Token![,]>()?;
            }
        }
        Ok(())
    };
    parser.parse2(attribute)?;

    if let (Some(span), None) = (planned, &output) {
        return Err(syn::Error::new(
            span,
            "`planned` makes the tree a planned command, and a command needs its output: \
             add `output = Type`, the type every leaf's output converts into",
        ));
    }
    if let Some((_, span)) = &context {
        if output.is_none() {
            return Err(syn::Error::new(
                *span,
                "`context` hands the context to the leaf that runs, so the tree runs: add \
                 `output = Type`, the type every leaf's output converts into",
            ));
        }
        if planned.is_some() {
            return Err(syn::Error::new(
                *span,
                "with `context`, the tree previews through each leaf's own \
                 `Handler::preview` — a planned leaf describes itself and any other refuses — \
                 so `planned` has nothing to add: drop it",
            ));
        }
    }
    let input = input.ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            format!(
                "`#[{ATTRIBUTE}]` needs the request it converts to: add \
                 `#[{ATTRIBUTE}(input = YourRequest)]`, an enum with a variant per subcommand"
            ),
        )
    })?;
    Ok(Declared {
        input,
        output,
        planned,
        context: context.map(|(ty, _)| ty),
    })
}

/// The leaf type's span for an arm that binds values from it.
///
/// The *location* is the leaf, so an error about it points at the variant; the *hygiene*
/// is the call site, so the bindings an arm introduces resolve in the caller's crate. That
/// second half matters when the leaf type was itself named by a macro (`clifacet::commands!`):
/// a span from another expansion cannot be seen from the caller's code.
fn arm_span(leaf: &Type) -> Span {
    Span::call_site().located_at(leaf.span())
}

/// The leaf of a `Name(Leaf)` variant, or what to write instead.
fn leaf_of(variant: &syn::Variant) -> syn::Result<Type> {
    let name = &variant.ident;
    let advice = format!(
        "write a leaf struct instead: `{name}({name})`, with \
         `#[clifacet::command(input = {name}Input)] struct {name} {{ … }}`"
    );
    match &variant.fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(fields.unnamed[0].ty.clone()),
        Fields::Named(_) => Err(syn::Error::new_spanned(
            variant,
            format!(
                "`{name}` declares its arguments inline, so it has no struct to validate them \
                 or to run: {advice}"
            ),
        )),
        Fields::Unit => Err(syn::Error::new_spanned(
            variant,
            format!(
                "`{name}` has no leaf, so it has no input to validate or command to run — even \
                 one without arguments needs one: {advice}, which may be empty"
            ),
        )),
        Fields::Unnamed(_) => Err(syn::Error::new_spanned(
            variant,
            format!("`{name}` holds more than one value, and a subcommand is one leaf: {advice}"),
        )),
    }
}

/// Replaces a variant's `#[clifacet(..)]` with the `#[command(..)]` it stands for, and
/// returns the names it declared, as `(key, name, span)`.
fn forward_variant_attributes(
    attributes: &mut Vec<Attribute>,
) -> syn::Result<Vec<(String, String, Span)>> {
    let mut names = Vec::new();
    let mut forwarded = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for attribute in attributes.drain(..) {
        if !attribute.path().is_ident(VARIANT_ATTRIBUTE) {
            forwarded.push(attribute);
            continue;
        }
        let entries = attribute.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for entry in entries {
            let (key, span) = match entry.path().get_ident() {
                Some(ident) => (ident.to_string(), ident.span()),
                None => {
                    return Err(syn::Error::new_spanned(
                        &entry,
                        unknown_variant_key("this key"),
                    ));
                }
            };
            if seen.contains(&key) {
                return Err(syn::Error::new(
                    span,
                    format!("`{key}` is declared twice on this variant; one key, one meaning"),
                ));
            }
            seen.push(key.clone());
            forwarded.push(variant_entry(&entry, &key, span, &mut names)?);
        }
    }
    *attributes = forwarded;
    Ok(names)
}

/// One `#[clifacet(..)]` entry on a variant, as the `#[command(..)]` clap reads.
fn variant_entry(
    entry: &Meta,
    key: &str,
    span: Span,
    names: &mut Vec<(String, String, Span)>,
) -> syn::Result<Attribute> {
    match entry {
        Meta::NameValue(pair) if VARIANT_VALUE_KEYS.contains(&key) => {
            let Some(text) = string(&pair.value) else {
                return Err(syn::Error::new_spanned(
                    &pair.value,
                    format!("`{key}` takes a string, for example `{key} = \"…\"`"),
                ));
            };
            names.push((key.to_owned(), text, span));
            let (path, value) = (&pair.path, &pair.value);
            Ok(syn::parse_quote!(#[command(#path = #value)]))
        }
        Meta::Path(path) if VARIANT_FLAG_KEYS.contains(&key) => {
            Ok(syn::parse_quote!(#[command(#path = true)]))
        }
        _ if VARIANT_VALUE_KEYS.contains(&key) => Err(syn::Error::new(
            span,
            format!("`{key}` takes a string, for example `{key} = \"…\"`"),
        )),
        _ if VARIANT_FLAG_KEYS.contains(&key) => Err(syn::Error::new(
            span,
            format!("`{key}` takes no value: write `{key}` on its own"),
        )),
        _ => Err(syn::Error::new(
            span,
            unknown_variant_key(&format!("`{key}`")),
        )),
    }
}

fn unknown_variant_key(key: &str) -> String {
    format!(
        "{key} is not a variant key of `#[{VARIANT_ATTRIBUTE}(..)]`; supported: {}, {}. \
         Any other clap key stays available as a raw `#[command(..)]` on the variant",
        VARIANT_VALUE_KEYS
            .iter()
            .map(|key| format!("`{key} = \"…\"`"))
            .collect::<Vec<_>>()
            .join(", "),
        VARIANT_FLAG_KEYS
            .iter()
            .map(|key| format!("`{key}`"))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

/// The names a raw `#[command(..)]` on a variant declares, so the duplicate check sees
/// them too. Anything that does not parse is clap's to report.
fn raw_names(attributes: &[Attribute]) -> Vec<(String, String, Span)> {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("command"))
        .filter_map(|attribute| {
            attribute
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .ok()
        })
        .flatten()
        .filter_map(|entry| match entry {
            Meta::NameValue(pair) => {
                let key = pair.path.get_ident()?.to_string();
                let text = string(&pair.value)?;
                NAMING_KEYS
                    .contains(&key.as_str())
                    .then(|| (key, text, pair.path.span()))
            }
            _ => None,
        })
        .collect()
}

/// Whether the enum renames its variants itself, so the default names are not kebab-case.
fn renames_all(attribute: &Attribute) -> bool {
    attribute.path().is_ident("command")
        && attribute
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .is_ok_and(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.path().is_ident("rename_all"))
            })
}

fn string(value: &Expr) -> Option<String> {
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) => Some(text.value()),
        _ => None,
    }
}

/// Rejects a `Subcommand` derive this attribute would emit a second time.
fn reject_duplicate_derive(attributes: &[Attribute]) -> syn::Result<()> {
    for attribute in attributes.iter().filter(|a| a.path().is_ident("derive")) {
        let Ok(paths) =
            attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
        else {
            continue;
        };
        if paths.iter().any(|path| {
            path.segments
                .last()
                .is_some_and(|last| last.ident == "Subcommand")
        }) {
            return Err(syn::Error::new_spanned(
                attribute,
                format!(
                    "`Subcommand` is derived twice: `#[{ATTRIBUTE}(..)]` already emits \
                     `clap::Subcommand`, so drop it from this derive"
                ),
            ));
        }
    }
    Ok(())
}

/// `TryFrom<Tree> for Request`, and the tree marker the `subcommands` field converts by.
///
/// Every `Request` variant is named once per leaf, spanned at the variant: a missing one
/// is "no variant named …" there, a mistyped one is a type mismatch at the leaf. An
/// extra one is caught by the match below, which names every variant the tree knows and
/// so is not exhaustive when `Request` has one more.
fn conversion(name: &Ident, input: &Type, leaves: &[Leaf]) -> TokenStream {
    let arms = leaves.iter().map(|Leaf { variant, leaf }| {
        let validated =
            quote_spanned!(arm_span(leaf)=> <#leaf as ::clifacet::Facet>::validate(leaf)?);
        quote_spanned!(arm_span(leaf)=> #name::#variant(leaf) => Self::#variant(#validated))
    });
    let every = leaves
        .iter()
        .map(|Leaf { variant, .. }| quote!(Self::#variant(_) => {}));
    let exhaustive = quote_spanned! {input.span()=>
        let _every_request_has_a_subcommand = |request: &Self| match request {
            #(#every)*
        };
    };
    quote! {
        impl ::core::convert::TryFrom<#name> for #input {
            type Error = ::clifacet::Failure;

            fn try_from(tree: #name) -> ::core::result::Result<Self, ::clifacet::Failure> {
                #exhaustive
                ::core::result::Result::Ok(match tree {
                    #(#arms,)*
                })
            }
        }

        impl ::clifacet::facet::__derive::Tree for #name {
            type Input = #input;

            fn into_input(self) -> ::core::result::Result<#input, ::clifacet::Failure> {
                <#input as ::core::convert::TryFrom<Self>>::try_from(self)
            }
        }
    }
}

/// `Collect` for the tree: the chosen leaf asks its own questions.
fn collect(name: &Ident, leaves: &[Leaf]) -> TokenStream {
    let arms = leaves.iter().map(|Leaf { variant, leaf }| {
        let collected = quote!(
            <#leaf as ::clifacet::prompt::Collect>::collect(leaf, session)?
        );
        quote_spanned!(arm_span(leaf)=> Self::#variant(leaf) => Self::#variant(#collected))
    });
    quote! {
        impl ::clifacet::prompt::Collect for #name {
            fn collect(
                self,
                session: &mut ::clifacet::prompt::Session,
            ) -> ::core::result::Result<Self, ::clifacet::Failure> {
                ::core::result::Result::Ok(match self {
                    #(#arms,)*
                })
            }
        }
    }
}

/// The message for an input built from another tree value than the one it runs on.
const MISMATCH: &str = "this request is for a different subcommand from the one that was parsed: \
                        build it from the same tree value with `TryFrom`";

/// `Command` for the tree: the chosen leaf runs with its own input, its output converted.
fn command(name: &Ident, input: &Type, output: &Type, leaves: &[Leaf]) -> TokenStream {
    let arms = leaves.iter().map(|Leaf { variant, leaf }| {
        let run = quote!(
            <#leaf as ::clifacet::Command>::execute(leaf, input).map(::core::convert::Into::into)
        );
        quote_spanned!(arm_span(leaf)=> (Self::#variant(leaf), __ClifacetRequest::#variant(input)) => #run)
    });
    quote! {
        impl ::clifacet::Command for #name {
            type Input = #input;
            type Output = #output;

            // A tree of one leaf has no other pair, so the fallback arm is unreachable; and
            // a leaf whose output already is the tree's converts to itself.
            #[allow(unreachable_patterns, clippy::useless_conversion)]
            fn execute(&self, input: #input) -> ::core::result::Result<#output, ::clifacet::Failure> {
                type __ClifacetRequest = #input;
                match (self, input) {
                    #(#arms,)*
                    (_, _) => ::core::result::Result::Err(::clifacet::Failure::execution(#MISMATCH)),
                }
            }
        }
    }
}

/// `Planned` for the tree: the chosen leaf's plan, its output converted when it runs.
fn planned(name: &Ident, input: &Type, output: &Type, leaves: &[Leaf]) -> TokenStream {
    let arms = leaves.iter().map(|Leaf { variant, leaf }| {
        let plan = quote!(
            <#leaf as ::clifacet::command::Planned>::plan(leaf, input)
                .map(|plan| plan.map(::core::convert::Into::into))
        );
        quote_spanned!(arm_span(leaf)=> (Self::#variant(leaf), __ClifacetRequest::#variant(input)) => #plan)
    });
    quote! {
        impl ::clifacet::command::Planned for #name {
            type Input = #input;
            type Output = #output;

            // A tree of one leaf has no other pair, so the fallback arm is unreachable; and
            // a leaf whose output already is the tree's converts to itself.
            #[allow(unreachable_patterns, clippy::useless_conversion)]
            fn plan(
                &self,
                input: #input,
            ) -> ::core::result::Result<::clifacet::command::Plan<'_, #output>, ::clifacet::Failure> {
                type __ClifacetRequest = #input;
                match (self, input) {
                    #(#arms,)*
                    (_, _) => ::core::result::Result::Err(::clifacet::Failure::execution(#MISMATCH)),
                }
            }
        }
    }
}

/// `Handler<Cx>` for the tree: the chosen leaf runs, or previews, with the context.
///
/// Each leaf is reached through its own `Handler<Cx>`: an application's handler, a
/// `#[clifacet::planned]` one, or any `Command` through the library's blanket impl.
fn handler(
    name: &Ident,
    input: &Type,
    output: &Type,
    context: &Type,
    leaves: &[Leaf],
) -> TokenStream {
    let handled = leaves.iter().map(|Leaf { variant, leaf }| {
        let run = quote!(
            <#leaf as ::clifacet::Handler<#context>>::handle(leaf, input, cx)
                .map(::core::convert::Into::into)
        );
        quote_spanned!(arm_span(leaf)=> (Self::#variant(leaf), __ClifacetRequest::#variant(input)) => #run)
    });
    let previewed = leaves.iter().map(|Leaf { variant, leaf }| {
        let plan = quote!(
            <#leaf as ::clifacet::Handler<#context>>::preview(leaf, input, cx)
                .map(|plan| plan.map(::core::convert::Into::into))
        );
        quote_spanned!(arm_span(leaf)=> (Self::#variant(leaf), __ClifacetRequest::#variant(input)) => #plan)
    });
    quote! {
        impl ::clifacet::Handler<#context> for #name {
            type Input = #input;
            type Output = #output;

            // As for `Command`: one leaf leaves the fallback unreachable, and a leaf whose
            // output already is the tree's converts to itself.
            #[allow(unreachable_patterns, clippy::useless_conversion)]
            fn handle(
                &self,
                input: #input,
                cx: &#context,
            ) -> ::core::result::Result<#output, ::clifacet::Failure> {
                type __ClifacetRequest = #input;
                match (self, input) {
                    #(#handled,)*
                    (_, _) => ::core::result::Result::Err(::clifacet::Failure::execution(#MISMATCH)),
                }
            }

            #[allow(unreachable_patterns, clippy::useless_conversion)]
            fn preview<'__clifacet>(
                &'__clifacet self,
                input: #input,
                cx: &'__clifacet #context,
            ) -> ::core::result::Result<
                ::clifacet::command::Plan<'__clifacet, #output>,
                ::clifacet::Failure,
            > {
                type __ClifacetRequest = #input;
                match (self, input) {
                    #(#previewed,)*
                    (_, _) => ::core::result::Result::Err(::clifacet::Failure::execution(#MISMATCH)),
                }
            }
        }
    }
}
