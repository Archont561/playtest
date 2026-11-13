// `#[clifacet::planned]` bridges `PlannedWith` only; on any other impl it says what it is for.
use clifacet::{Command, Failure};

struct Add;

#[clifacet::planned]
impl Command for Add {
    type Input = String;
    type Output = usize;

    fn execute(&self, input: String) -> Result<usize, Failure> {
        Ok(input.len())
    }
}

fn main() {}
