//! One module, one command: listing it twice is a compile error, not a doubled variant.

mod commands {
    clifacet::commands!(greet, greet);
}

fn main() {}
