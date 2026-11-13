//! A correctly declared derive compiles.
use clifacet::Facet;

#[derive(clifacet::clap::Parser, Facet)]
#[facet(input = Greeting)]
struct Cli {
    #[arg(long, alias = "nickname", short = 'n')]
    name: Option<String>,

    /// A credential from the environment compiles once its value is hidden from help.
    #[arg(long, env = "GREETING_TOKEN", hide_env_values = true)]
    #[facet(secret)]
    token: Option<clifacet::secret::Secret<String>>,
}

/// The typed input the derive builds field by field.
struct Greeting {
    name: Option<String>,
    token: Option<clifacet::secret::Secret<String>>,
}

fn main() {
    let _ = <Cli as Facet>::command();
}
