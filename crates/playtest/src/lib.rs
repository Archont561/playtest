//! # playtest
//!
//! Polyglot test framework with the batteries included: dependency-injected fixtures,
//! strict LIFO teardown, property-based testing, and code-first BDD for Rust.

#[doc(hidden)]
pub mod __private;
pub mod fixture;
#[doc(hidden)]
pub mod skip;

#[allow(unused_imports)]
pub use playtest_macros::*;

pub use fixture::{
    AsyncFixture, BoxFuture, FixtureHandle, FixtureStack, TeardownHook, TeardownReport,
    run_with_teardown,
};

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
