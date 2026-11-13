//! A unit variant has no leaf, even when it takes no arguments.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
    Done,
}

enum Request {
    Add(AddInput),
    Done(DoneInput),
}

#[clifacet::command(input = AddInput)]
struct Add {
    title: String,
}

struct AddInput {
    title: String,
}

#[clifacet::command(input = DoneInput)]
struct Done {
    id: u32,
}

struct DoneInput {
    id: u32,
}

fn main() {}
