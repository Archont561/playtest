//! A skipped field never reaches the input, so a rule on it would do nothing.

#[clifacet::command(input = Greeting)]
struct Cli {
    #[clifacet(long)]
    name: Option<String>,

    #[clifacet(long, skip, trim)]
    note: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
