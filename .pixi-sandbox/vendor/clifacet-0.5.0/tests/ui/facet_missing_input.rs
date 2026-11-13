#[derive(clifacet::clap::Parser, clifacet::Facet)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
}

fn main() {}
