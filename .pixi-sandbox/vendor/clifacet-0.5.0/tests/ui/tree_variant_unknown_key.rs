//! A variant key the surface does not define is named in the error.

#[clifacet::subcommands(input = Request)]
enum Commands {
    #[clifacet(about = "Add a task")]
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
