//! `#[clifacet::command(...)]`: clap's parser and the input bridge in one attribute.
//!
//! The pairing this wraps is two attributes that have to agree:
//!
//! ```ignore
//! #[derive(clap::Parser, clifacet::Facet)]
//! #[facet(input = Greeting)]
//! struct Cli { .. }
//! ```
//!
//! A derive macro cannot add another derive to the item it is attached to — derives only
//! contribute impls — so this is an attribute macro: it re-emits the item exactly as it
//! was, with both derives and the `#[facet(...)]` attribute in front of it. That is the
//! whole trick, and it is why the wrapper cannot drift from the derive: it emits the
//! derive rather than a second implementation of it.
//!
//! One consequence is worth stating, because it is the one thing this cannot hide:
//! `clap`'s own derive expands to code that names `clap::…`, so the calling crate still
//! needs `clap` as a direct dependency. `clifacet` re-exports `clap` so an application
//! never ends up on a different major version than the derive was built against, but the
//! crate name has to resolve where the derive lands.

use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Attribute, Data, DeriveInput, Fields, Token};

/// The derives this attribute stands for, in the order they are emitted.
const PARSER: &str = "Parser";
const FACET: &str = "Facet";

pub(crate) fn expand(attribute: &TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    // A function is the same command with its parameters as the declaration: the arm hands the
    // item to its own module, which emits the struct the derive below would have wrapped.
    if let syn::Item::Fn(function) = syn::parse2::<syn::Item>(item.clone())? {
        return crate::function::expand(attribute, quote!(#function));
    }

    let mut parsed = syn::parse2::<DeriveInput>(item)?;
    if !matches!(parsed.data, Data::Struct(_)) {
        return Err(syn::Error::new_spanned(
            &parsed,
            "`#[clifacet::command]` applies to a struct: the input bridge copies fields \
             by name, and clap's subcommand enums have no fields to copy",
        ));
    }

    if !crate::facet::declares_input(attribute.clone())? {
        return Err(syn::Error::new_spanned(
            &parsed,
            "`#[clifacet::command]` needs the typed input to build: \
             add `#[clifacet::command(input = YourInputType)]`, and a \
             `bridge = path` if the input normalizes on the way in",
        ));
    }

    // Fields first: a bad `#[clifacet(..)]` should be reported as this attribute's
    // mistake, before the item is emitted and clap starts complaining about the result.
    forward_field_attributes(&mut parsed)?;

    let mut attributes = parsed.attrs.clone();
    reject_duplicates(&attributes)?;

    // The caller's own attributes stay in front of the item, untouched: `#[command(..)]`,
    // `#[arg(..)]`, `#[doc]` and any derive of theirs are clap's and Rust's business.
    attributes.insert(
        0,
        syn::parse_quote!(#[derive(::clap::Parser, ::clifacet::Facet)]),
    );
    attributes.insert(1, syn::parse_quote!(#[facet(#attribute)]));

    parsed.attrs = attributes;

    Ok(quote!(#parsed))
}

/// Replaces every field's `#[clifacet(..)]` with the `#[arg(..)]` it stands for.
///
/// The replacement is the whole mechanism of decision-13: the interaction and
/// normalization keys are validated here and consumed, the grammar keys are forwarded to
/// clap as written, and a raw `#[arg(..)]` on the same field is left exactly where it is
/// so the two compose — one declaration, one parser.
fn forward_field_attributes(parsed: &mut DeriveInput) -> syn::Result<()> {
    let Data::Struct(data) = &mut parsed.data else {
        // The struct check above already rejected everything else with a better message.
        return Ok(());
    };
    let Fields::Named(fields) = &mut data.fields else {
        return Ok(());
    };

    for field in &mut fields.named {
        let mut forwarded = Vec::new();
        for attribute in field.attrs.drain(..) {
            if attribute.path().is_ident(crate::field::ATTRIBUTE) {
                let declared = crate::field::parse(&attribute, &field.ty)?;
                forwarded.extend(declared.attributes());
            } else {
                forwarded.push(attribute);
            }
        }
        field.attrs = forwarded;
    }
    Ok(())
}

/// Rejects the shapes that would make the emitted derives collide with something already
/// there, rather than letting a duplicate impl surface as a type error later.
fn reject_duplicates(attributes: &[Attribute]) -> syn::Result<()> {
    for attribute in attributes {
        if attribute.path().is_ident("facet") {
            return Err(syn::Error::new_spanned(
                attribute,
                "`#[facet(..)]` is declared twice: `#[clifacet::command(..)]` emits it, \
                 so its keys belong in that attribute instead",
            ));
        }
        if !attribute.path().is_ident("derive") {
            continue;
        }
        for derive in derived_names(attribute) {
            if derive == PARSER || derive == FACET {
                return Err(syn::Error::new_spanned(
                    attribute,
                    format!(
                        "`{derive}` is derived twice: `#[clifacet::command(..)]` already \
                         emits `clap::Parser` and `clifacet::Facet`, so keep this derive \
                         and drop the attribute, or the reverse"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// The last path segment of every name in `#[derive(..)]`, which is how a derive written
/// as `clap::Parser` and one written as `Parser` are recognized as the same derive.
fn derived_names(attribute: &Attribute) -> Vec<String> {
    let Ok(entries) =
        attribute.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated)
    else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|path| {
            path.segments
                .last()
                .map(|segment| segment.ident.to_string())
        })
        .collect()
}
