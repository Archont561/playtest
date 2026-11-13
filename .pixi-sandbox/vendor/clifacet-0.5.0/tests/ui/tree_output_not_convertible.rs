//! A leaf's output that does not convert into the tree's.

use clifacet::{Command, Failure};

#[clifacet::subcommands(input = Request, output = Report)]
enum Commands {
    Done(Done),
}

/// What the tree reports; nothing converts a leaf's `u32` into it.
struct Report;

enum Request {
    Done(DoneInput),
}

#[clifacet::command(input = DoneInput)]
struct Done {
    id: u32,
}

struct DoneInput {
    id: u32,
}

impl Command for Done {
    type Input = DoneInput;
    type Output = u32;

    fn execute(&self, input: DoneInput) -> Result<u32, Failure> {
        Ok(input.id)
    }
}

fn main() {}
