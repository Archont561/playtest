use clifacet::Facet;

#[derive(clifacet::clap::Parser, Facet)]
#[facet(input = Greeting, magic = true)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
