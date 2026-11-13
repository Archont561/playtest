//! What `input = Type` may be: any type the generated impls can name.
//!
//! The input type is written into three places — `type Input = …`, the generated
//! `impl TryFrom<Cli> for …` and `validate`'s return type — so it has to be a type that
//! means the same thing in all three. Most types do, and they are accepted as written:
//! a qualified path, a generic type, an alias, a projection such as
//! `<Marker as Shape>::Input`. The rest are rejected here, with the error on the tokens
//! at fault, because the compiler would otherwise report them once per generated item
//! and in terms of code the caller never wrote:
//!
//! - `Self`, which inside those impls is the command struct or the input itself;
//! - a lifetime the command struct does not declare, and the elided `'_`;
//! - `_` and `impl Trait`, which leave the one type `validate` returns unnamed;
//! - without `bridge = path`, a type that is not a struct path: the generated bridge
//!   builds the input with a struct literal.

use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{BoundLifetimes, Generics, Ident, Lifetime, Type};

/// Rejects an input type the generated impls could not name, as one combined error.
pub(crate) fn check(input: &Type, generics: &Generics, bridged: bool) -> syn::Result<()> {
    let mut scan = Scan {
        declared: generics
            .lifetimes()
            .map(|param| &param.lifetime.ident)
            .collect(),
        bound: Vec::new(),
        errors: Vec::new(),
    };
    scan.visit_type(input);
    if !bridged && !names_a_struct(input) {
        scan.errors.push(syn::Error::new_spanned(
            input,
            "without `bridge = path`, the generated bridge builds the input field by field, so \
             `input` has to name a struct; declare `bridge = path` to build this type yourself",
        ));
    }
    scan.errors
        .into_iter()
        .reduce(|mut first, next| {
            first.combine(next);
            first
        })
        .map_or(Ok(()), Err)
}

/// Whether a struct literal can build the type: a path, possibly parenthesized. A type
/// macro is given the benefit of the doubt, since only its expansion knows.
fn names_a_struct(input: &Type) -> bool {
    match input {
        Type::Path(_) | Type::Macro(_) => true,
        Type::Paren(inner) => names_a_struct(&inner.elem),
        Type::Group(inner) => names_a_struct(&inner.elem),
        _ => false,
    }
}

/// Walks the input type, recording every token the generated impls could not name.
struct Scan<'a> {
    /// The lifetimes the command struct declares, which its impls declare too.
    declared: Vec<&'a Ident>,
    /// The lifetimes a `for<..>` around the current position binds.
    bound: Vec<Ident>,
    errors: Vec<syn::Error>,
}

impl Scan<'_> {
    /// Visits `walk` with the lifetimes of `binder` in scope, as `for<'b>` puts them.
    fn with_bound(&mut self, binder: Option<&BoundLifetimes>, walk: impl FnOnce(&mut Self)) {
        let outer = self.bound.len();
        self.bound.extend(
            binder
                .into_iter()
                .flat_map(|binder| binder.lifetimes.iter())
                .filter_map(|param| match param {
                    syn::GenericParam::Lifetime(param) => Some(param.lifetime.ident.clone()),
                    _ => None,
                }),
        );
        walk(self);
        self.bound.truncate(outer);
    }

    fn in_scope(&self, name: &Ident) -> bool {
        name == "static" || self.declared.contains(&name) || self.bound.contains(name)
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_path_segment(&mut self, segment: &'ast syn::PathSegment) {
        if segment.ident == "Self" {
            self.errors.push(syn::Error::new_spanned(
                &segment.ident,
                "`input` cannot mention `Self`: the generated impls are for the command struct, \
                 so `Self` would be the command or the input itself; name the type directly",
            ));
        }
        visit::visit_path_segment(self, segment);
    }

    fn visit_lifetime(&mut self, lifetime: &'ast Lifetime) {
        if lifetime.ident == "_" {
            self.errors.push(syn::Error::new_spanned(
                lifetime,
                "`input` needs every lifetime named: `'_` cannot appear in an associated type; \
                 declare a lifetime on the command struct or use `'static`",
            ));
        } else if !self.in_scope(&lifetime.ident) {
            self.errors.push(syn::Error::new_spanned(
                lifetime,
                format!(
                    "`{lifetime}` is not declared on the command struct, so the generated impls \
                     cannot name it: declare it there (`struct Cli<{lifetime}>`) or use `'static`"
                ),
            ));
        }
    }

    fn visit_type_infer(&mut self, infer: &'ast syn::TypeInfer) {
        self.errors.push(unnamed(infer, "_"));
    }

    fn visit_type_impl_trait(&mut self, opaque: &'ast syn::TypeImplTrait) {
        self.errors.push(unnamed(opaque, "impl Trait"));
    }

    fn visit_type_bare_fn(&mut self, function: &'ast syn::TypeBareFn) {
        self.with_bound(function.lifetimes.as_ref(), |scan| {
            visit::visit_type_bare_fn(scan, function);
        });
    }

    fn visit_trait_bound(&mut self, bound: &'ast syn::TraitBound) {
        self.with_bound(bound.lifetimes.as_ref(), |scan| {
            visit::visit_trait_bound(scan, bound);
        });
    }
}

/// The error for a type that leaves the input to inference, over all of its tokens.
fn unnamed(tokens: impl ToTokens, written: &str) -> syn::Error {
    syn::Error::new_spanned(
        tokens,
        format!(
            "`input` names the one type `validate` returns, so `{written}` cannot stand for it: \
             write the concrete type"
        ),
    )
}
