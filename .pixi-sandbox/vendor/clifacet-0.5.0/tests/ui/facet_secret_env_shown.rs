//! clap's help prints an environment variable's value, so a secret read from one would
//! be shown by `--help`.
use clifacet::Facet;
use clifacet::secret::Secret;

#[derive(clifacet::clap::Parser, Facet)]
#[facet(input = Login)]
struct Cli {
    #[arg(long, env = "TOKEN")]
    #[facet(secret)]
    token: Option<Secret<String>>,
}

struct Login {
    token: Option<Secret<String>>,
}

fn main() {}
