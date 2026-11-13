#[derive(clifacet::Facet)]
#[facet(input = Greeting)]
enum Cli {
    Run,
}

struct Greeting;

fn main() {}
