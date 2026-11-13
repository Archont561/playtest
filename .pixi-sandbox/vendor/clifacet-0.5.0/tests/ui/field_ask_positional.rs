//! An answer is parsed as a flag's value, so only a named option can be asked for.

#[clifacet::command(input = Greeting)]
struct Cli {
    #[clifacet(value_name = "NAME", ask = "Who?")]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
