//! `playtest-macros` is a `proc-macro` crate. Rust permits such a crate to export macros
//! only, so it carries no `VERSION` constant: its manifest is the version authority, and
//! `require-aligned-manifest-versions` pins it to the workspace version.
//!
//! The macros themselves are exercised end to end from the `playtest` crate, which is the
//! only place they are meant to be called:
//!
//! - `crates/playtest/tests/macro_test.rs`
//! - `crates/playtest/tests/skip_test.rs`

#[test]
fn the_macro_crate_compiles_as_a_proc_macro_library() {
    // Reaching this line proves the crate built as a proc-macro library; the macro surface
    // is covered by the expansion tests linked above.
}
