//! `Request` has a variant no subcommand produces.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
    Done(Done),
}

enum Request {
    Add(AddInput),
    Done(DoneInput),
    Archive(DoneInput),
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
