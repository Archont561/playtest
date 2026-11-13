//! The bridge is field by field, so an input missing a field is a compile error.


#[derive(clifacet::clap::Parser, clifacet::Facet)]
#[facet(input = Greeting)]
struct Cli {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    repeat: u8,
}

struct Greeting {
    name: Option<String>,
}

fn main() {}
