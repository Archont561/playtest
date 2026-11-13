//! A variant with inline fields has no struct to validate or run them.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add { title: String },
    Done(Done),
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
