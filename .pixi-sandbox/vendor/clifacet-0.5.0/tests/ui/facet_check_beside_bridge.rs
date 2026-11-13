//! A declared bridge builds the whole input, so it runs its own checks.

use clifacet::Failure;

#[derive(clap::Parser, clifacet::Facet)]
#[facet(input = Greeting, bridge = build, check = check)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn build(cli: Cli) -> Result<Greeting, Failure> {
    Ok(Greeting { name: cli.name })
}

fn check(_: &Greeting) -> Result<(), Failure> {
    Ok(())
}

fn main() {}
