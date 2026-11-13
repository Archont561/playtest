//! A declared `collect` asks every question, so a field's `ask` would never be asked.

use clifacet::prompt::Session;
use clifacet::Failure;

#[clifacet::command(input = Greeting, collect = collect)]
struct Cli {
    #[clifacet(long, ask = "Who?")]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn collect(cli: Cli, _: &mut Session) -> Result<Cli, Failure> {
    Ok(cli)
}

fn main() {}
