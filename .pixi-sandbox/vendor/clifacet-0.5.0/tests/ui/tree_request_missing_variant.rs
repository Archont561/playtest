//! `Request` has no variant for a subcommand the tree declares.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
    Done(Done),
}

enum Request {
    Add(AddInput),
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
