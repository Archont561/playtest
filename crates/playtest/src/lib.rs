//! # playtest
//!
//! Polyglot test framework with the batteries included: dependency-injected fixtures,
//! strict LIFO teardown, property-based testing, and code-first BDD for Rust.

#[allow(unused_imports)]
pub use playtest_macros::*;

/// Current crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
