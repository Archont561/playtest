// A type is a `Handler` through `Command` already; a second, hand-written `Handler` would
// disagree with it about what runs, so the two conflict.
use clifacet::{Command, Failure, Handler};

struct Store;
struct Add;

impl Command for Add {
    type Input = String;
    type Output = usize;

    fn execute(&self, input: String) -> Result<usize, Failure> {
        Ok(input.len())
    }
}

impl Handler<Store> for Add {
    type Input = String;
    type Output = usize;

    fn handle(&self, input: String, _store: &Store) -> Result<usize, Failure> {
        Ok(input.len())
    }
}

fn main() {}
