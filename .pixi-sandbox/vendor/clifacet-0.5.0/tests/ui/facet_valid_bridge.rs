//! A declared bridge compiles and wins over the field-by-field mirror.
use clifacet::{Facet, Failure};

#[derive(clifacet::clap::Parser, Facet)]
#[facet(input = Greeting, bridge = build_greeting)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
}

/// The input normalizes, so it deliberately shares no field type with `Cli`.
struct Greeting {
    name: String,
}

fn build_greeting(cli: Cli) -> Result<Greeting, Failure> {
    Ok(Greeting {
        name: cli.name.unwrap_or_else(|| String::from("world")),
    })
}

fn main() {
    let cli = <Cli as clifacet::clap::Parser>::parse_from(["greet", "--name", "ada"]);
    let input = cli.validate().expect("a bridge that accepts everything cannot fail");
    assert_eq!(input.name, "ada");
}
