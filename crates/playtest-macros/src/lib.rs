//! # playtest-macros
//!
//! Procedural macros for playtest, re-exported from the `playtest` crate.
//!
//! The macros generate a [`#[test]`](https://doc.rust-lang.org/reference/attributes/testing.html)
//! function from a declaration, and route the fixtures through the engine in `playtest`
//! through the `$crate`-equivalent path `::playtest`. Test code depends only on `playtest`,
//! so that path is always available where the macro expands.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Ident, LitStr, Token, braced, parse_macro_input};

/// A parsed `test!` declaration (`Spec 0002 §1`).
struct TestDeclaration {
    /// The test name, reported by the runner and on failure.
    name: LitStr,
    /// The fixtures the body requests, in resolution order.
    fixtures: Vec<Ident>,
    /// The async body.
    body: syn::Block,
}

impl Parse for TestDeclaration {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;

        let content;
        braced!(content in input);
        let fixtures = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();

        input.parse::<Token![=>]>()?;
        let body: syn::Block = input.parse()?;
        Ok(Self {
            name,
            fixtures,
            body,
        })
    }
}

/// Declare an async test that requests fixtures by name (`Spec 0002 §1`).
///
/// ```ignore
/// playtest::test!("user registration", { db, tmpdir } => {
///     let user = db.create_user("Alice").await;
///     assert!(user.is_valid());
/// });
/// ```
///
/// Each requested name is resolved through the fixture engine and torn down in reverse
/// order afterwards. Fixtures that the body does not name are never constructed
/// (`Spec 0001 §3.1`), and teardown runs even when the body panics (`Spec 0001 §3.2`).
///
/// The expansion binds every fixture to a local of the same name, wraps the body in an
/// async block, and hands the whole thing to `playtest::fixture::run_test`.
#[proc_macro]
pub fn test(input: TokenStream) -> TokenStream {
    let TestDeclaration {
        name,
        fixtures,
        body,
    } = parse_macro_input!(input as TestDeclaration);
    expand_test(&name, &fixtures, &body)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Turn a test title into the name of the function the macro generates.
///
/// Several `test!` declarations share one module, so the name has to be unique per title;
/// a title of `"user registration"` becomes `__playtest_user_registration`.
fn function_name(name: &LitStr) -> Ident {
    let mut sanitized = String::from("__playtest_");
    let mut last_underscore = true;
    for character in name.value().chars() {
        if character.is_ascii_alphanumeric() {
            sanitized.push(character.to_ascii_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            sanitized.push('_');
            last_underscore = true;
        }
    }
    sanitized.truncate(sanitized.trim_end_matches('_').len());
    if sanitized.ends_with('_') || sanitized == "__playtest_" {
        sanitized.push_str("case");
    }
    Ident::new(&sanitized, name.span())
}

/// Build the function that a `test!` declaration expands into.
fn expand_test(name: &LitStr, fixtures: &[Ident], body: &syn::Block) -> syn::Result<TokenStream2> {
    let function = function_name(name);
    let setup: Vec<TokenStream2> = fixtures
        .iter()
        .map(|fixture| {
            quote! {
                #[allow(non_snake_case, unused_variables)]
                let #fixture = stack.setup::<#fixture>().await;
            }
        })
        .collect();
    let requested = fixtures.len();

    Ok(quote! {
        #[test]
        fn #function() {
            ::playtest::__private::block_on(async {
                let mut stack = ::playtest::FixtureStack::new();
            #(#setup)*
            ::playtest::__private::assert_fixture_count(stack.len(), #requested);
                let body = async move #body;
                let (_value, report) = ::playtest::run_with_teardown(&mut stack, body).await;
                ::playtest::__private::report_teardown_failures(&report, #name);
            });
        }
    })
}

/// A parsed `test_skip!` declaration (`Spec 0002 §1`).
struct SkipDeclaration {
    /// The test name, included in the recorded reason.
    name: LitStr,
    /// Why the test is skipped, so the skip is actionable rather than mysterious.
    reason: LitStr,
}

impl Parse for SkipDeclaration {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let reason: LitStr = input.parse()?;
        Ok(Self { name, reason })
    }
}

/// An unconditional skip (`Spec 0002 §1`).
///
/// ```ignore
/// playtest::test_skip!("windows only", "requires a Windows host");
/// ```
///
/// The expansion registers a `#[test]` that records why it did not run, so a skipped test
/// is never mistaken for a passing one.
#[proc_macro]
pub fn test_skip(input: TokenStream) -> TokenStream {
    let SkipDeclaration { name, reason } = parse_macro_input!(input as SkipDeclaration);
    let function = function_name(&name);
    quote! {
        #[test]
        fn #function() {
            println!("{}", ::playtest::skip::record(#name, #reason).report_line());
        }
    }
    .into()
}

/// A parsed `skip_if!` declaration (`Spec 0002 §1`).
struct SkipIfDeclaration {
    name: LitStr,
    condition: syn::Expr,
}

impl Parse for SkipIfDeclaration {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        input.parse::<Token![,]>()?;
        let condition: syn::Expr = input.parse()?;
        Ok(Self { name, condition })
    }
}

/// A runtime conditional skip (`Spec 0002 §1`).
///
/// ```ignore
/// playtest::skip_if!("unix only", cfg!(unix));
/// ```
///
/// The condition is evaluated when the test runs, not when it compiles, so a skip can
/// depend on the host's actual state.
#[proc_macro]
pub fn skip_if(input: TokenStream) -> TokenStream {
    let SkipIfDeclaration { name, condition } = parse_macro_input!(input as SkipIfDeclaration);
    let function = function_name(&name);
    quote! {
        #[test]
        fn #function() {
            let condition = #condition;
            if ::playtest::skip::should_skip(condition) {
                let skip = ::playtest::skip::record(
                    #name,
                    ::playtest::skip::condition_reason(condition),
                );
                println!("{}", skip.report_line());
            }
        }
    }
    .into()
}
