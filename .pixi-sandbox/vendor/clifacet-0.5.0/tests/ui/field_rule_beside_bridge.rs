//! A declared bridge owns normalization, so a field rule beside it would do nothing.

use clifacet::Failure;

#[clifacet::command(input = Greeting, bridge = build)]
struct Cli {
    #[clifacet(long, trim)]
    name: Option<String>,
}

struct Greeting {
    name: String,
}

fn build(cli: Cli) -> Result<Greeting, Failure> {
    Ok(Greeting { name: cli.name.unwrap_or_default() })
}

fn main() {}
