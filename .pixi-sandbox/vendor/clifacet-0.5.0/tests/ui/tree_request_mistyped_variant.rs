//! A `Request` variant holds another input than its leaf builds.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
    Done(Done),
}

enum Request {
    Add(AddInput),
    Done(AddInput),
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
