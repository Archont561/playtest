//! A name two subcommands answer to is reported at the second one.

#[clifacet::subcommands(input = Request)]
enum Commands {
    Add(Add),
    #[clifacet(alias = "add")]
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
