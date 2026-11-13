//! A rule beside `subcommands` would apply to the whole tree.

#[clifacet::command(input = Input)]
struct Cli {
    #[clifacet(subcommands, skip)]
    command: Option<Commands>,
}

struct Input {}
#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
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
