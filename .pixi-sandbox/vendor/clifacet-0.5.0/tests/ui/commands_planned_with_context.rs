//! A tree with a context previews through its leaves, so `planned` has nothing to add.

mod commands {
    clifacet::commands!(greet, planned, context = Store);
}

fn main() {}
