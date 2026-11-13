//! `#[clifacet::planned]`: the `Handler` a `PlannedWith` impl implies (decision-19 item 2).
//!
//! `Planned` gets `Command` from a blanket impl in the library. Its twin with a context,
//! `PlannedWith<Cx>`, cannot get `Handler<Cx>` the same way: that blanket would overlap the
//! one every `Command` already has (E0119). So the bridge is written here, beside the impl
//! it comes from, and it is the same bridge the blanket would have been — `handle` executes
//! the plan and `preview` returns it — so a dry run and a real run still share one path.

use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned as _;
use syn::{GenericArgument, ItemImpl, PathArguments, Type};

/// The attribute's own name, for messages.
const ATTRIBUTE: &str = "clifacet::planned";

// The item is returned alongside errors, so the macro keeps it rather than dropping it:
// taking it by reference would copy it out for nothing.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn expand(attribute: &TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if !attribute.is_empty() {
        return Err(syn::Error::new(
            attribute.span(),
            format!("`#[{ATTRIBUTE}]` takes no arguments: the impl it is on says everything"),
        ));
    }
    let Ok(implementation) = syn::parse2::<ItemImpl>(item.clone()) else {
        return Err(syn::Error::new(item.span(), wrong_item()));
    };
    let context = context_of(&implementation)?;
    let self_type = &implementation.self_ty;
    let (generics, _, where_clause) = implementation.generics.split_for_impl();
    Ok(quote! {
        #implementation

        impl #generics ::clifacet::Handler<#context> for #self_type #where_clause {
            type Input = <Self as ::clifacet::command::PlannedWith<#context>>::Input;
            type Output = <Self as ::clifacet::command::PlannedWith<#context>>::Output;

            fn handle(
                &self,
                input: Self::Input,
                cx: &#context,
            ) -> ::core::result::Result<Self::Output, ::clifacet::Failure> {
                ::clifacet::command::PlannedWith::<#context>::plan(self, input, cx)?.execute()
            }

            fn preview<'__clifacet>(
                &'__clifacet self,
                input: Self::Input,
                cx: &'__clifacet #context,
            ) -> ::core::result::Result<
                ::clifacet::command::Plan<'__clifacet, Self::Output>,
                ::clifacet::Failure,
            > {
                ::clifacet::command::PlannedWith::<#context>::plan(self, input, cx)
            }
        }
    })
}

/// The `Cx` of `impl PlannedWith<Cx> for T`, or an error saying what the attribute is for.
fn context_of(implementation: &ItemImpl) -> syn::Result<&Type> {
    let Some((negated, path, _)) = &implementation.trait_ else {
        return Err(syn::Error::new(implementation.self_ty.span(), wrong_item()));
    };
    let last = path.segments.last();
    if negated.is_some() || last.is_none_or(|segment| segment.ident != "PlannedWith") {
        return Err(syn::Error::new(path.span(), wrong_item()));
    }
    let arguments = last.map(|segment| &segment.arguments);
    if let Some(PathArguments::AngleBracketed(arguments)) = arguments
        && let [GenericArgument::Type(context)] =
            arguments.args.iter().collect::<Vec<_>>().as_slice()
    {
        return Ok(context);
    }
    Err(syn::Error::new(
        path.span(),
        "`PlannedWith` names the context it plans with: write `impl PlannedWith<Store> for …`",
    ))
}

fn wrong_item() -> String {
    format!(
        "`#[{ATTRIBUTE}]` goes on an `impl PlannedWith<Context> for Command` block, and adds \
         the `Handler<Context>` that executes its plan; a planned command without a context \
         implements `Planned` and needs no attribute"
    )
}
