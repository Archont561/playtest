//! A variant's leaf is a plain clap struct, with no input to validate into.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
}

enum Request {
    Add(AddInput),
}

#[derive(clap::Args)]
struct Add {
    title: String,
}

struct AddInput {
    title: String,
}

fn main() {}
