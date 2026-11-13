//! A command file has to hold its struct (CLI-50, AC#4).
//!
//! `<name>.rs` is a command named `<name>` whose struct is `Name`, and `commands!`
//! re-exports that struct under the name the tree uses. A file that holds something else
//! fails where the application wrote the call — the module import and the missing type,
//! both pointing at this line — rather than compiling into a tree with a hole in it.
//!
//! Under discovery the same two errors come from the same two generated items, with `broken`
//! read from the folder instead of from a list, so this case covers both paths.
// `broken.rs` sits beside this file and holds `SomethingElse` rather than `Broken`.
mod commands {
    clifacet::commands!(broken);
}

fn main() {}
